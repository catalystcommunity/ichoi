plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

// Release inputs. scripts/release-bundle.sh sets them from the version and the secret store.
val ichoiVersionName = providers.gradleProperty("ichoiVersionName").orNull ?: "0.1.0-dev"
val ichoiVersionCode = providers.gradleProperty("ichoiVersionCode").orNull?.toInt() ?: 1
val uploadKeystore = providers.environmentVariable("ICHOI_UPLOAD_KEYSTORE").orNull

android {
    namespace = "community.catalyst.ichoi"
    compileSdk = 36

    defaultConfig {
        // AND-00: the permanent Play application ID. It cannot change after publication.
        applicationId = "com.catalystsquad.ichoi"
        minSdk = 29
        targetSdk = 36
        versionCode = ichoiVersionCode
        versionName = ichoiVersionName
    }

    signingConfigs {
        if (uploadKeystore != null) {
            create("upload") {
                storeFile = file(uploadKeystore)
                storePassword = providers.environmentVariable("ICHOI_UPLOAD_KEYSTORE_PASSWORD").get()
                keyAlias = providers.environmentVariable("ICHOI_UPLOAD_KEY_ALIAS").get()
                keyPassword = providers.environmentVariable("ICHOI_UPLOAD_KEY_PASSWORD").get()
            }
        }
    }

    buildTypes {
        getByName("debug") {
            // Development-only. A debug build must never use the store ID.
            applicationIdSuffix = ".dev"
        }
        getByName("release") {
            isMinifyEnabled = false
            // Unsigned without the upload key. A Play bundle must be signed.
            signingConfig = signingConfigs.findByName("upload")
        }
    }

    buildFeatures { buildConfig = true }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }

    sourceSets["main"].java.srcDir("../../server/generated/kotlin-client")
    sourceSets["test"].java.srcDir("src/test/java")
}

val permanentApplicationId = "com.catalystsquad.ichoi"
tasks.register("verifyStoreRelease") {
    val releaseId = android.defaultConfig.applicationId
    doLast {
        check(releaseId == permanentApplicationId) {
            "Release application ID $releaseId is not the permanent ID $permanentApplicationId"
        }
    }
}
tasks.register("verifyUploadSigning") {
    val signed = uploadKeystore != null
    doLast {
        check(signed) { "bundleRelease needs the upload key. Use scripts/release-bundle.sh." }
    }
}
tasks.matching { it.name == "assembleRelease" || it.name == "bundleRelease" }
    .configureEach { dependsOn("verifyStoreRelease") }
tasks.matching { it.name == "bundleRelease" }.configureEach { dependsOn("verifyUploadSigning") }
tasks.register("auditDependencies") {
    doLast {
        val forbidden = Regex("analytics|advertis|billing|crashlytics|sentry|firebase|updater|remote.?code", RegexOption.IGNORE_CASE)
        listOf("debugRuntimeClasspath", "releaseRuntimeClasspath").forEach { name ->
            configurations.getByName(name).incoming.resolutionResult.allComponents.forEach { component ->
                component.moduleVersion?.let { version ->
                    val module = "${version.group}:${version.name}"
                    check(!forbidden.containsMatchIn(module)) { "Prohibited Android dependency: $module" }
                }
            }
        }
    }
}
tasks.register<Exec>("androidCompliance") {
    dependsOn("auditDependencies")
    commandLine(rootProject.file("scripts/check_compliance.sh").absolutePath)
    workingDir(rootProject.projectDir)
}
tasks.named("check") { dependsOn("androidCompliance") }

dependencies {
    implementation(kotlin("stdlib"))
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    testImplementation(kotlin("test"))
    testImplementation("org.junit.jupiter:junit-jupiter:5.11.0")
}

tasks.withType<Test>().configureEach { useJUnitPlatform() }
