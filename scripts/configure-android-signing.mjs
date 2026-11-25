#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

/**
 * 在 CI 中为生成的 Android 工程写入签名配置。
 *
 * 用法：
 *   node scripts/configure-android-signing.mjs <keystore-path>
 *
 * 需要环境变量：
 *   ANDROID_KEY_ALIAS
 *   ANDROID_KEY_PASSWORD
 */

function fail(message) {
  console.error(`[android-signing] ${message}`);
  process.exit(1);
}

function escapePropertyValue(value) {
  // Java properties 中反斜杠是转义符，需要成对写入
  return value.replace(/\\/g, "\\\\");
}

const projectRoot = process.cwd();
const keystoreArg = process.argv[2] ?? process.env.ANDROID_KEYSTORE_PATH;
const keyAlias = process.env.ANDROID_KEY_ALIAS;
const keyPassword = process.env.ANDROID_KEY_PASSWORD;

if (!keystoreArg) {
  fail("缺少 keystore 路径，请作为参数传入或设置 ANDROID_KEYSTORE_PATH");
}
if (!keyAlias) {
  fail("缺少环境变量 ANDROID_KEY_ALIAS");
}
if (!keyPassword) {
  fail("缺少环境变量 ANDROID_KEY_PASSWORD");
}

const keystorePath = path.resolve(keystoreArg);
if (!fs.existsSync(keystorePath)) {
  fail(`找不到 keystore 文件：${keystorePath}`);
}

const androidDir = path.join(projectRoot, "src-tauri", "gen", "android");
const gradlePath = path.join(androidDir, "app", "build.gradle.kts");
if (!fs.existsSync(gradlePath)) {
  fail("找不到 Android Gradle 工程，请先执行 tauri android init");
}

const propertiesPath = path.join(androidDir, "keystore.properties");
const storePath = keystorePath.replace(/\\/g, "/");
const propertiesContent = [
  `password=${escapePropertyValue(keyPassword)}`,
  `keyAlias=${escapePropertyValue(keyAlias)}`,
  `storeFile=${escapePropertyValue(storePath)}`,
  "",
].join("\n");
fs.writeFileSync(propertiesPath, propertiesContent, "utf8");
console.log(`[android-signing] 已写入 ${path.relative(projectRoot, propertiesPath)}`);

let gradle = fs.readFileSync(gradlePath, "utf8");
let changed = false;

if (!gradle.includes("import java.util.Properties")) {
  gradle = `import java.util.Properties\n${gradle}`;
  changed = true;
}

if (!gradle.includes("import java.io.FileInputStream")) {
  gradle = `import java.io.FileInputStream\n${gradle}`;
  changed = true;
}

const signingConfigsBlock = `    signingConfigs {
        create("release") {
            val keystorePropertiesFile = rootProject.file("keystore.properties")
            val keystoreProperties = Properties()
            if (keystorePropertiesFile.exists()) {
                keystoreProperties.load(FileInputStream(keystorePropertiesFile))
            }
            keyAlias = keystoreProperties["keyAlias"] as String
            keyPassword = keystoreProperties["password"] as String
            storeFile = file(keystoreProperties["storeFile"] as String)
            storePassword = keystoreProperties["password"] as String
        }
    }
`;

if (!gradle.includes("signingConfigs {")) {
  const androidMatch = gradle.match(/^android \{/m);
  if (!androidMatch || androidMatch.index === undefined) {
    fail("未找到 android { 配置块，无法写入签名配置");
  }
  const insertAt = androidMatch.index + androidMatch[0].length;
  gradle = `${gradle.slice(0, insertAt)}\n${signingConfigsBlock}${gradle.slice(insertAt)}`;
  changed = true;
}

if (!/signingConfig\s*=\s*signingConfigs\.getByName\("release"\)/.test(gradle)) {
  const releaseMatch = gradle.match(/getByName\("release"\)\s*\{\s*\n/);
  if (!releaseMatch || releaseMatch.index === undefined) {
    fail("未找到 release buildType，无法应用签名配置");
  }
  const insertAt = releaseMatch.index + releaseMatch[0].length;
  gradle = `${gradle.slice(0, insertAt)}            signingConfig = signingConfigs.getByName("release")\n${gradle.slice(insertAt)}`;
  changed = true;
}

if (changed) {
  fs.writeFileSync(gradlePath, gradle, "utf8");
  console.log("[android-signing] 已更新 app/build.gradle.kts");
} else {
  console.log("[android-signing] Gradle 已包含签名配置，跳过修改");
}
