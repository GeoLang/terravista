package dev.geolang.terravista.testapp;

import android.content.Context;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Rect;
import android.graphics.RectF;
import android.util.Log;
import android.view.MotionEvent;
import android.view.View;

import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * Draws a raster map whose every geometric decision comes from terravista.
 * This class only fetches bytes, decodes them, and blits them where the SDK says.
 */
public class MapView extends View {
    private static final String TAG = "TerraVistaTest";
    private static final String TILE_URL = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
    private static final String USER_AGENT =
            "TerraVistaAndroidTest/0.1 (+https://github.com/GeoLang/terravista)";
    private static final int MAX_DECODED_TILES = 128;
    /** openstreetmap.org serves no tiles past z19. */
    private static final double MAX_SOURCE_ZOOM = 19.0;
    /** How many zoom levels up to search for a stand-in while a tile loads. */
    private static final int MAX_PARENT_LEVELS = 4;
    /** Movement below this still counts as a tap, which rotates the map. */
    private static final float TAP_SLOP_PX = 24f;

    /** The SDK is not thread-safe, so every FFI call holds this lock. */
    private final Object sdk = new Object();
    private long handle;

    private final ExecutorService fetchers = Executors.newFixedThreadPool(4);
    private final Set<Long> inFlight = ConcurrentHashMap.newKeySet();

    private final Map<Long, Bitmap> decoded =
            new LinkedHashMap<Long, Bitmap>(16, 0.75f, true) {
                @Override
                protected boolean removeEldestEntry(Map.Entry<Long, Bitmap> eldest) {
                    return size() > MAX_DECODED_TILES;
                }
            };

    private final Paint tilePaint = new Paint(Paint.FILTER_BITMAP_FLAG);
    private final Paint textPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint shadowPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint panelPaint = new Paint();

    // scratch buffers so onDraw allocates nothing per tile
    private final int[] zxy = new int[3];
    private final float[] xys = new float[3];
    private final int[] range = new int[5];
    private final RectF dst = new RectF();
    private final Rect src = new Rect();

    /** Camera zoom above which the SDK would ask for tiles the source lacks. */
    private final double maxCameraZoom;

    private int lastGesture = TerraVista.GESTURE_NONE;
    private int frame = 0;
    private int lastVisibleCount = 0;
    /** API 35 draws edge to edge, so keep the readout clear of the status bar. */
    private int topInset = 0;

    private float downX;
    private float downY;
    private boolean dragged;

    public MapView(Context context) {
        super(context);
        float dpr = getResources().getDisplayMetrics().density;
        // the SDK biases tile zoom by the density, so cap the camera below that
        maxCameraZoom = MAX_SOURCE_ZOOM - (Math.log(dpr) / Math.log(2.0));

        synchronized (sdk) {
            handle = TerraVista.create(1, 1, dpr);
            TerraVista.setTileUrl(handle, TILE_URL);
            TerraVista.setCenter(handle, 51.5074, -0.1278);
            TerraVista.setZoom(handle, 12.0);
            Log.i(TAG, "terravista " + TerraVista.version() + " handle=" + handle + " dpr=" + dpr);
        }

        textPaint.setColor(Color.WHITE);
        textPaint.setTextSize(13f * dpr);
        shadowPaint.setColor(Color.BLACK);
        shadowPaint.setTextSize(13f * dpr);
        panelPaint.setColor(Color.argb(170, 0, 0, 0));
        setBackgroundColor(Color.rgb(233, 229, 220));
    }

    @Override
    protected void onSizeChanged(int w, int h, int oldW, int oldH) {
        super.onSizeChanged(w, h, oldW, oldH);
        synchronized (sdk) {
            TerraVista.setViewport(handle, w, h, getResources().getDisplayMetrics().density);
        }
        Log.i(TAG, "viewport " + w + "x" + h);
    }

    @Override
    public android.view.WindowInsets onApplyWindowInsets(android.view.WindowInsets insets) {
        topInset = insets.getSystemWindowInsetTop();
        return super.onApplyWindowInsets(insets);
    }

    @Override
    public boolean onTouchEvent(MotionEvent event) {
        int phase;
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN:
                downX = event.getX();
                downY = event.getY();
                dragged = false;
                phase = TerraVista.TOUCH_BEGIN;
                break;
            case MotionEvent.ACTION_POINTER_DOWN:
                dragged = true; // a second finger is never a tap
                phase = TerraVista.TOUCH_BEGIN;
                break;
            case MotionEvent.ACTION_MOVE:
                if (Math.hypot(event.getX() - downX, event.getY() - downY) > TAP_SLOP_PX) {
                    dragged = true;
                }
                phase = TerraVista.TOUCH_MOVE;
                break;
            case MotionEvent.ACTION_UP:
                if (!dragged) {
                    rotateByQuarterTurn();
                }
                phase = TerraVista.TOUCH_END;
                break;
            case MotionEvent.ACTION_POINTER_UP:
                // hand the recognizer a fresh Begin with the fingers still down,
                // so lifting one of two keeps panning with the other
                sendTouch(TerraVista.TOUCH_BEGIN, event, event.getActionIndex());
                return true;
            case MotionEvent.ACTION_CANCEL:
                phase = TerraVista.TOUCH_CANCEL;
                break;
            default:
                return true;
        }
        sendTouch(phase, event, -1);
        return true;
    }

    /** Forward every live pointer to the SDK recognizer, skipping {@code skipIndex}. */
    private void sendTouch(int phase, MotionEvent event, int skipIndex) {
        int total = event.getPointerCount();
        int n = (skipIndex >= 0) ? total - 1 : total;
        double[] xs = new double[n];
        double[] ys = new double[n];
        long[] ids = new long[n];

        int out = 0;
        for (int i = 0; i < total; i++) {
            if (i == skipIndex) {
                continue;
            }
            xs[out] = event.getX(i);
            ys[out] = event.getY(i);
            ids[out] = event.getPointerId(i);
            out++;
        }

        synchronized (sdk) {
            lastGesture = TerraVista.touch(handle, phase, xs, ys, ids);
            // stop a pinch from running past the deepest zoom OSM serves
            if (TerraVista.getZoom(handle) > maxCameraZoom) {
                TerraVista.setZoom(handle, maxCameraZoom);
            }
        }
        if (phase == TerraVista.TOUCH_END) {
            logState("touch-end", readoutLines(lastVisibleCount));
        }
        invalidate();
    }

    @Override
    protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);

        int count;
        double bearing;
        synchronized (sdk) {
            count = TerraVista.visibleTileCount(handle);
            bearing = TerraVista.getBearing(handle);
        }
        lastVisibleCount = count;

        // placements come back north-up, so spin the canvas to face the bearing
        canvas.save();
        if (bearing != 0.0) {
            canvas.rotate((float) -bearing, getWidth() / 2f, getHeight() / 2f);
        }

        for (int i = 0; i < count; i++) {
            boolean ok;
            synchronized (sdk) {
                ok = TerraVista.visibleTileAt(handle, i, zxy, xys);
            }
            if (!ok) {
                continue;
            }

            int z = zxy[0], x = zxy[1], y = zxy[2];
            long key = tileKey(z, x, y);
            Bitmap bmp = decoded.get(key);

            if (bmp == null) {
                byte[] bytes;
                synchronized (sdk) {
                    bytes = TerraVista.cacheGet(handle, z, x, y);
                }
                if (bytes != null) {
                    bmp = BitmapFactory.decodeByteArray(bytes, 0, bytes.length);
                    if (bmp != null) {
                        decoded.put(key, bmp);
                    }
                } else {
                    requestTile(z, x, y, key);
                }
            }

            dst.set(xys[0], xys[1], xys[0] + xys[2], xys[1] + xys[2]);
            if (bmp != null) {
                canvas.drawBitmap(bmp, null, dst, tilePaint);
            } else {
                drawParentTile(canvas, z, x, y);
            }
        }

        canvas.restore();

        drawReadout(canvas, count);
    }

    /** Tapping rotates 45 degrees, which is the only way to drive bearing from adb. */
    private void rotateByQuarterTurn() {
        synchronized (sdk) {
            TerraVista.setBearing(handle, (TerraVista.getBearing(handle) + 45.0) % 360.0);
        }
    }

    /**
     * Blit the matching crop of an already-decoded lower-zoom tile into {@link #dst},
     * so panning shows blurry map instead of blank white while tiles load.
     */
    private void drawParentTile(Canvas canvas, int z, int x, int y) {
        for (int up = 1; up <= MAX_PARENT_LEVELS && z - up >= 0; up++) {
            Bitmap parent = decoded.get(tileKey(z - up, x >> up, y >> up));
            if (parent == null) {
                continue;
            }

            int span = 1 << up;
            int cell = parent.getWidth() / span;
            if (cell < 1) {
                return;
            }
            int left = (x & (span - 1)) * cell;
            int top = (y & (span - 1)) * cell;
            src.set(left, top, left + cell, top + cell);
            canvas.drawBitmap(parent, src, dst, tilePaint);
            return;
        }
    }

    private void logState(String why, String[] lines) {
        Log.i(TAG, why + "  " + lines[0] + " | " + lines[1] + " | " + lines[2] + " | " + lines[3]);
    }

    /** Every value on the panel is read back from the SDK, nothing is cached in Java. */
    private String[] readoutLines(int visibleCount) {
        double zoom, lat, lon, bearing, pitch;
        int cached, rangeOk;
        long bytes;
        synchronized (sdk) {
            zoom = TerraVista.getZoom(handle);
            lat = TerraVista.getCenterLat(handle);
            lon = TerraVista.getCenterLon(handle);
            bearing = TerraVista.getBearing(handle);
            pitch = TerraVista.getPitch(handle);
            cached = TerraVista.cacheTileCount(handle);
            bytes = TerraVista.cacheSizeBytes(handle);
            rangeOk = TerraVista.tileRange(handle, range) ? 1 : 0;
        }

        return new String[] {
            String.format("zoom %.2f  bearing %.1f  pitch %.1f", zoom, bearing, pitch),
            String.format("center %.5f, %.5f", lat, lon),
            rangeOk == 1
                    ? String.format(
                            "z%d x%d-%d y%d-%d  visible %d",
                            range[0], range[1], range[2], range[3], range[4], visibleCount)
                    : "tile range unavailable",
            String.format(
                    "sdk cache %d tiles / %d KB  gesture %s",
                    cached, bytes / 1024, gestureName(lastGesture)),
        };
    }

    private void drawReadout(Canvas canvas, int visibleCount) {
        String[] lines = readoutLines(visibleCount);

        if (frame++ % 30 == 0) {
            logState("frame " + frame, lines);
        }

        float pad = textPaint.getTextSize() * 0.5f;
        float lineHeight = textPaint.getTextSize() * 1.35f;
        canvas.drawRect(
                0, topInset, getWidth(), topInset + pad * 2 + lineHeight * lines.length, panelPaint);

        float y = topInset + pad + textPaint.getTextSize();
        for (String line : lines) {
            canvas.drawText(line, pad + 1, y + 1, shadowPaint);
            canvas.drawText(line, pad, y, textPaint);
            y += lineHeight;
        }
    }

    private static String gestureName(int g) {
        switch (g) {
            case TerraVista.GESTURE_PAN:
                return "pan";
            case TerraVista.GESTURE_ZOOM:
                return "zoom";
            case TerraVista.GESTURE_ROTATE:
                return "rotate";
            case TerraVista.GESTURE_PITCH:
                return "pitch";
            default:
                return "none";
        }
    }

    private static long tileKey(int z, int x, int y) {
        return ((long) z << 58) | ((long) x << 29) | (long) y;
    }

    private void requestTile(int z, int x, int y, long key) {
        if (!inFlight.add(key)) {
            return;
        }
        final String url;
        synchronized (sdk) {
            url = TerraVista.tileUrl(handle, z, x, y);
        }
        if (url == null || url.isEmpty()) {
            inFlight.remove(key);
            return;
        }

        fetchers.submit(
                () -> {
                    try {
                        byte[] body = httpGet(url);
                        if (body != null) {
                            synchronized (sdk) {
                                TerraVista.cachePut(handle, z, x, y, body, "image/png");
                            }
                            postInvalidate();
                        }
                    } catch (Exception e) {
                        Log.w(TAG, "tile " + z + "/" + x + "/" + y + " failed: " + e);
                    } finally {
                        inFlight.remove(key);
                    }
                });
    }

    private static byte[] httpGet(String url) throws Exception {
        HttpURLConnection conn = (HttpURLConnection) new URL(url).openConnection();
        try {
            conn.setRequestProperty("User-Agent", USER_AGENT);
            conn.setConnectTimeout(10000);
            conn.setReadTimeout(15000);

            int code = conn.getResponseCode();
            if (code != 200) {
                Log.w(TAG, "HTTP " + code + " for " + url);
                return null;
            }

            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] chunk = new byte[8192];
            try (InputStream in = conn.getInputStream()) {
                int read;
                while ((read = in.read(chunk)) != -1) {
                    out.write(chunk, 0, read);
                }
            }
            return out.toByteArray();
        } finally {
            conn.disconnect();
        }
    }

    public void release() {
        fetchers.shutdownNow();
        synchronized (sdk) {
            if (handle != 0) {
                TerraVista.destroy(handle);
                handle = 0;
            }
        }
    }
}
