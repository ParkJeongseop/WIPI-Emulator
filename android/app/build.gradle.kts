import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

// 서명 설정은 keystore.properties(gitignore)에서 읽는다. 없으면 release도 debug 키로 서명(로컬 확인용).
val keystorePropsFile = rootProject.file("keystore.properties")
val keystoreProps = Properties().apply {
    if (keystorePropsFile.exists()) load(keystorePropsFile.inputStream())
}

android {
    // 2026-07-21 wie 명칭 제거: namespace/JNI 심볼도 com.parkjeongseop.wipi로 통일
    namespace = "com.parkjeongseop.wipi"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.parkjeongseop.wipi"
        minSdk = 26
        targetSdk = 36
        versionCode = 4  // 매 업로드 증가 필요
        versionName = "0.3.0"
        // AAB는 ABI별로 자동 분할되므로 실기기(arm64-v8a) 슬라이스만 배포됨.
        // x86_64는 에뮬레이터용이라 Play 배포 AAB엔 불필요하지만, 둘 다 넣어도 무방(APK로 뽑을 때 편함).
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    signingConfigs {
        if (keystorePropsFile.exists()) {
            create("release") {
                storeFile = rootProject.file(keystoreProps.getProperty("storeFile"))
                storePassword = keystoreProps.getProperty("storePassword")
                keyAlias = keystoreProps.getProperty("keyAlias")
                keyPassword = keystoreProps.getProperty("keyPassword")
            }
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    androidResources {
        localeFilters += listOf("en", "ko")
    }

    buildTypes {
        release {
            isMinifyEnabled = true       // R8: 코드 축소·난독화
            isShrinkResources = true     // 미사용 리소스 제거 (material-icons-extended로 커진 크기 대응)
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
            signingConfig = if (keystorePropsFile.exists()) {
                signingConfigs.getByName("release")
            } else {
                signingConfigs.getByName("debug") // keystore 없으면 로컬 확인용 debug 서명
            }
        }
    }

    buildFeatures {
        compose = true
        buildConfig = true // 설정 화면의 버전 표시용
    }
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2024.12.01"))
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended") // SF Symbols 대응 아이콘 (이모지 대체)
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
}
