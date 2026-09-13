plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "community.catalyst.ichoi"
    compileSdk = 36

    defaultConfig {
        // AND-00 is intentionally unresolved. This ID is development-only.
        applicationId = "community.catalyst.ichoi.dev"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0-dev"
    }

    buildTypes {
        getByName("debug") {
            applicationIdSuffix = ".debug"
        }
        getByName("release") {
            isMinifyEnabled = false
            // Store publishing must fail until AND-00 selects a permanent ID.
            buildConfigField("Boolean", "STORE_RELEASE_ENABLED", "false")
            manifestPlaceholders["storeReleaseEnabled"] = false
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

tasks.register("verifyStoreRelease") {
    doLast {
        check(false) {
            "Store release is blocked: AND-00 permanent application ID is not selected"
        }
    }
}
tasks.matching { it.name == "assembleRelease" }.configureEach { dependsOn("verifyStoreRelease") }
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
