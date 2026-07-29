package dev.geolang.terravista.testapp;

import android.app.Activity;
import android.os.Bundle;

public class MainActivity extends Activity {
    private MapView map;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        map = new MapView(this);
        setContentView(map);
    }

    @Override
    protected void onDestroy() {
        map.release();
        super.onDestroy();
    }
}
