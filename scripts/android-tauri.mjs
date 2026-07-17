import { existsSync, readdirSync } from "node:fs"
import { homedir, platform } from "node:os"
import { join } from "node:path"
import { spawnSync } from "node:child_process"

const command = process.argv[2]
const args = process.argv.slice(3)

if (!command || !["dev", "build"].includes(command)) {
  console.error("Usage: bun run android:dev | bun run android:build [tauri options]")
  process.exit(1)
}

function findNdk() {
  const explicit = [process.env.ANDROID_NDK_HOME, process.env.ANDROID_NDK_ROOT].filter(Boolean)
  const sdkRoots = [
    process.env.ANDROID_HOME,
    process.env.ANDROID_SDK_ROOT,
    platform() === "darwin" ? join(homedir(), "Library/Android/sdk") : join(homedir(), "Android/Sdk"),
  ].filter(Boolean)

  const versioned = sdkRoots.flatMap((sdkRoot) => {
    const ndkRoot = join(sdkRoot, "ndk")
    if (!existsSync(ndkRoot)) return []
    return readdirSync(ndkRoot)
      .sort()
      .reverse()
      .map((version) => join(ndkRoot, version))
  })

  return [...explicit, ...versioned].find((path) => existsSync(join(path, "toolchains/llvm/prebuilt")))
}

function hostTag() {
  if (platform() === "darwin") return "darwin-x86_64"
  if (platform() === "linux") return "linux-x86_64"
  return "windows-x86_64"
}

const ndk = findNdk()
const sysroot = ndk && join(ndk, "toolchains/llvm/prebuilt", hostTag(), "sysroot")

if (!sysroot || !existsSync(sysroot)) {
  console.error("Android NDK not found. Install it or set ANDROID_NDK_HOME.")
  process.exit(1)
}

const env = { ...process.env }
const toolchainBin = join(ndk, "toolchains/llvm/prebuilt", hostTag(), "bin")
const bindgenTargets = {
  aarch64_linux_android: "aarch64-linux-android",
  armv7_linux_androideabi: "armv7a-linux-androideabi",
  i686_linux_android: "i686-linux-android",
  x86_64_linux_android: "x86_64-linux-android",
}
const apiLevel = process.env.ANDROID_API_LEVEL ?? "36"

for (const [envTarget, clangTarget] of Object.entries(bindgenTargets)) {
  const targetKey = envTarget.toUpperCase()
  const clang = join(toolchainBin, `${clangTarget}${apiLevel}-clang`)
  const clangxx = join(toolchainBin, `${clangTarget}${apiLevel}-clang++`)
  const targetAr = join(toolchainBin, "llvm-ar")
  if (!existsSync(clang) || !existsSync(clangxx) || !existsSync(targetAr)) {
    console.error(`Android NDK toolchain is incomplete for ${clangTarget} API ${apiLevel}`)
    process.exit(1)
  }
  env[`CC_${envTarget}`] = clang
  env[`CC_${clangTarget}`] = clang
  env[`CXX_${envTarget}`] = clangxx
  env[`CXX_${clangTarget}`] = clangxx
  env[`AR_${envTarget}`] = targetAr
  env[`AR_${clangTarget}`] = targetAr
  env[`CARGO_TARGET_${targetKey}_LINKER`] = clang
  const includeDir = clangTarget.startsWith("armv7")
    ? "arm-linux-androideabi"
    : clangTarget.split("-").slice(0, 1)[0]
  env[`BINDGEN_EXTRA_CLANG_ARGS_${envTarget}`] = [
    `--target=${clangTarget}${apiLevel}`,
    `--sysroot=${sysroot}`,
    `-isystem ${join(sysroot, "usr/include", includeDir)}`,
  ].join(" ")
}

const result = spawnSync("bun", ["tauri", "android", command, ...args], {
  env,
  stdio: "inherit",
})

process.exit(result.status ?? 1)
