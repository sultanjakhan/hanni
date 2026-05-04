import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

// Release signing — read from keystore.properties (local) or env vars (CI).
// File-based config is gitignored; env config is used by .github/workflows/release-android.yml.
val keystoreProps = Properties().apply {
    val f = file("../keystore.properties")
    if (f.exists()) f.inputStream().use { load(it) }
}
val ksFile: String? = keystoreProps.getProperty("storeFile")
    ?: System.getenv("ANDROID_KEYSTORE_PATH")
val ksPassword: String? = keystoreProps.getProperty("storePassword")
    ?: System.getenv("ANDROID_KEYSTORE_PASSWORD")
val ksKeyAlias: String? = keystoreProps.getProperty("keyAlias")
    ?: System.getenv("ANDROID_KEY_ALIAS")
val ksKeyPassword: String? = keystoreProps.getProperty("keyPassword")
    ?: System.getenv("ANDROID_KEY_PASSWORD")
val hasReleaseSigning = ksFile != null && ksPassword != null
    && ksKeyAlias != null && ksKeyPassword != null

android {
    compileSdk = 36
    namespace = "com.sultanjakhan.hanni"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "com.sultanjakhan.hanni"
        minSdk = 26
        targetSdk = 36
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
    }
    if (hasReleaseSigning) {
        signingConfigs {
            create("release") {
                // Local keystore.properties: storeFile is relative to gen/android/ (one level
                // above app/). CI: ANDROID_KEYSTORE_PATH is absolute.
                storeFile = if (ksFile!!.startsWith("/")) file(ksFile!!) else file("../$ksFile")
                storePassword = ksPassword
                keyAlias = ksKeyAlias
                keyPassword = ksKeyPassword
            }
        }
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            isMinifyEnabled = true
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
            if (hasReleaseSigning) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    sourceSets["main"].java.srcDirs("../../../android-plugin/src/main/java")
    buildFeatures {
        buildConfig = true
    }
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    // Health Connect (Samsung Health integration)
    implementation("androidx.health.connect:connect-client:1.1.0-alpha10")
    // Coroutines for Health Connect async calls
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.8.1")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")