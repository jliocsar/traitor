#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { chmodSync, copyFileSync, cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

const PLATFORMS = [
  { target: 'x86_64-unknown-linux-musl', os: 'linux', cpu: 'x64' },
  { target: 'aarch64-unknown-linux-musl', os: 'linux', cpu: 'arm64' },
  { target: 'x86_64-apple-darwin', os: 'darwin', cpu: 'x64' },
  { target: 'aarch64-apple-darwin', os: 'darwin', cpu: 'arm64' },
  { target: 'x86_64-pc-windows-msvc', os: 'win32', cpu: 'x64' },
]
const MAIN_PACKAGE_DIRECTORY = 'packages/traitor'
const RELEASE_DIRECTORY = 'release'
const PUBLISH_ORDER_FILE = 'publish-order.json'

function fail(message) {
  console.error(message)
  process.exit(1)
}

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'))
}

function writeJson(path, value) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`)
}

function executableName({ os }) {
  return os === 'win32' ? 'traitor.exe' : 'traitor'
}

function platformDirectoryName({ os, cpu }) {
  return `traitor-${os}-${cpu}`
}

function stage(version, artifactsDirectory, outputDirectory) {
  if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version ?? '')) {
    fail(`stage needs a version such as 1.2.3, not ${JSON.stringify(version)}`)
  }
  const expectedArtifacts = PLATFORMS.map((platform) => `traitor-${platform.target}`).sort()
  const foundArtifacts = readdirSync(artifactsDirectory).sort()
  if (JSON.stringify(expectedArtifacts) !== JSON.stringify(foundArtifacts)) {
    fail(`${artifactsDirectory} holds ${foundArtifacts.join(', ')}, but the release needs exactly ${expectedArtifacts.join(', ')}`)
  }

  const main = readJson(join(MAIN_PACKAGE_DIRECTORY, 'package.json'))
  rmSync(outputDirectory, { recursive: true, force: true })
  mkdirSync(join(outputDirectory, RELEASE_DIRECTORY), { recursive: true })

  const platformPackages = PLATFORMS.map((platform) => {
    const name = `${main.name}-${platform.os}-${platform.cpu}`
    const directory = join(outputDirectory, platformDirectoryName(platform))
    const executable = executableName(platform)
    const binary = join(artifactsDirectory, `traitor-${platform.target}`, executable)
    if (!existsSync(binary)) {
      fail(`${binary} is missing`)
    }
    mkdirSync(directory)
    copyFileSync(binary, join(directory, executable))
    chmodSync(join(directory, executable), 0o755)
    writeJson(join(directory, 'package.json'), {
      name,
      version,
      description: `The traitor binary for ${platform.os} ${platform.cpu}. Install ${main.name}, not this.`,
      license: main.license,
      repository: main.repository,
      os: [platform.os],
      cpu: [platform.cpu],
      files: [executable],
    })
    const releaseAsset = join(outputDirectory, RELEASE_DIRECTORY, `${platformDirectoryName(platform)}${platform.os === 'win32' ? '.exe' : ''}`)
    copyFileSync(binary, releaseAsset)
    chmodSync(releaseAsset, 0o755)
    return { name, directory }
  })

  const mainDirectory = join(outputDirectory, 'traitor')
  cpSync(MAIN_PACKAGE_DIRECTORY, mainDirectory, { recursive: true })
  for (const file of ['README.md', 'LICENSE']) {
    if (existsSync(file)) {
      copyFileSync(file, join(mainDirectory, file))
    }
  }
  writeJson(join(mainDirectory, 'package.json'), {
    ...main,
    version,
    optionalDependencies: Object.fromEntries(platformPackages.map(({ name }) => [name, version])),
  })

  const publishOrder = [...platformPackages, { name: main.name, directory: mainDirectory }]
  writeJson(join(outputDirectory, PUBLISH_ORDER_FILE), { version, packages: publishOrder })
  console.log(`staged ${publishOrder.length} packages and ${PLATFORMS.length} release binaries for ${version} in ${outputDirectory}`)
}

function isPublished(name, version) {
  try {
    execFileSync('npm', ['view', `${name}@${version}`, 'version'], { stdio: ['ignore', 'pipe', 'ignore'] })
    return true
  } catch {
    return false
  }
}

function publish(outputDirectory) {
  const { version, packages } = readJson(join(outputDirectory, PUBLISH_ORDER_FILE))
  const distributionTag = version.includes('-') ? 'next' : 'latest'
  for (const { name, directory } of packages) {
    if (isPublished(name, version)) {
      console.log(`${name}@${version} is already on npm`)
      continue
    }
    execFileSync('npm', ['publish', directory, '--access', 'public', '--tag', distributionTag], { stdio: 'inherit' })
  }
}

const [command, ...rest] = process.argv.slice(2)
const commands = {
  stage: () => stage(...rest),
  publish: () => publish(...rest),
}
if (!(command in commands)) {
  fail('usage: npm-release.mjs stage <version> <artifacts directory> <output directory> | publish <output directory>')
}
commands[command]()
