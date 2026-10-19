plugins {
    id("com.android.application")
    kotlin("android")
}

android {
    namespace = "dev.origiri.nitium"
    compileSdk = 34

    defaultConfig {
        applicationId = "dev.origiri.nitium"
        minSdk = 26
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }

    packaging {
        jniLibs {
            useLegacyPackaging = false
        }
    }

    signingConfigs {
        getByName("debug") {
            storeFile =
                file(
                    System.getenv("CARGO_APK_RELEASE_KEYSTORE")
                        ?: "${System.getProperty("user.home")}/.android/debug.keystore",
                )
            storePassword = System.getenv("CARGO_APK_RELEASE_KEYSTORE_PASSWORD") ?: "android"
            keyAlias = "androiddebugkey"
            keyPassword = System.getenv("CARGO_APK_RELEASE_KEYSTORE_PASSWORD") ?: "android"
        }
    }

    buildTypes {
        release {
            signingConfig = signingConfigs.getByName("debug")
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    implementation("androidx.core:core-splashscreen:1.0.1")
}
