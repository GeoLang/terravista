plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("maven-publish")
}

android {
    namespace = "dev.geolang.terravista"
    compileSdk = 35
    buildToolsVersion = "35.0.0"

    defaultConfig {
        minSdk = 24
        buildConfigField(
            "String",
            "LIB_VERSION",
            "\"${project.property("VERSION_NAME")}\"",
        )
    }

    buildFeatures {
        buildConfig = true
    }

    // the .so files under src/main/jniLibs are prebuilt, see tools/build-natives.sh
    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    publishing {
        singleVariant("release") {
            withSourcesJar()
        }
    }
}

afterEvaluate {
    publishing {
        publications {
            create<MavenPublication>("release") {
                from(components["release"])
                groupId = "com.github.GeoLang"
                artifactId = "terravista"
                version = project.property("VERSION_NAME") as String

                pom {
                    name.set("TerraVista")
                    description.set("Mobile map view backed by the TerraVista Rust core")
                    url.set("https://github.com/GeoLang/terravista")
                    licenses {
                        license {
                            name.set("AGPL-3.0-or-later")
                            url.set("https://www.gnu.org/licenses/agpl-3.0.txt")
                        }
                    }
                }
            }
        }
    }
}
