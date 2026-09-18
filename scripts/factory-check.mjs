import { execFileSync } from "node:child_process"
import { readFileSync } from "node:fs"
import process from "node:process"

const ranks = { low: 0, medium: 1, high: 2, critical: 3 }
const normalize = (file) => file.replaceAll("\\", "/").replace(/^\.\//, "")

export function classifyFiles(inputFiles) {
  const files = [...new Set(inputFiles.filter(Boolean).map(normalize))].sort()
  let risk = "low"
  const reviewers = new Set()
  const reasons = []
  const raise = (level, reason) => {
    if (ranks[level] > ranks[risk]) risk = level
    reasons.push(reason)
  }

  for (const file of files) {
    const lower = file.toLowerCase()
    const docsOnly = /^(?:docs?|context)\/.*\.(?:md|txt)$/.test(lower) || lower === "readme.md"
    if (/(^|\/)(?:\.env[^/]*|auth\.json|[^/]*\.pem)$/.test(lower) || ["scripts/factory-check.mjs", "scripts/factory-guard.mjs", "scripts/opencode_usage.py", "scripts/test_opencode_usage.py"].includes(lower) || /(^|\/)(agents\.md|opencode\.json)$/.test(lower) || /(^|\/)\.github\/workflows\//.test(lower) || /(^|\/)\.opencode\/(agents|commands|plugins|skills)\//.test(lower)) {
      raise("critical", `${file}: agent, permission, or CI control`)
      reviewers.add("architecture")
      reviewers.add("security")
    } else if (/(^|\/)(deploy|release|migration|auth|permission|security|credential)/.test(lower)) {
      raise("high", `${file}: high-impact operational surface`)
      reviewers.add("security")
    } else if (!docsOnly) {
      raise("medium", `${file}: executable or configuration behavior`)
    }
    if (!lower.startsWith(".opencode/") && /(^|\/)(finance|billing|invoice|ledger|currency|forecast|allocation)(\/|[._-])/.test(lower)) {
      raise("high", `${file}: financial behavior`)
      reviewers.add("finance")
    }
  }

  if (risk !== "low") reviewers.add("general")
  if (risk === "critical") reviewers.add("acceptance")
  return { files, risk, reviewers: [...reviewers].sort(), reasons: [...new Set(reasons)] }
}

const secretPatterns = [
  ["private key", /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/g],
  ["OpenAI-style token", /\bsk-[A-Za-z0-9_-]{20,}\b/g],
  ["GitHub token", /\bgh[pousr]_[A-Za-z0-9]{20,}\b/g],
  ["Slack token", /\bxox[baprs]-[A-Za-z0-9-]{20,}\b/g],
  ["assigned secret", /\b(?:api[_-]?key|client[_-]?secret|access[_-]?token|password|private[_-]?key)\b["']?\s*[:=]\s*["']([^"'{}\s]{16,})["']/gi],
]

export function scanSecretText(text) {
  const findings = []
  for (const [kind, pattern] of secretPatterns) {
    pattern.lastIndex = 0
    for (const match of text.matchAll(pattern)) {
      const line = text.slice(0, match.index).split("\n").length
      findings.push({ kind, line })
    }
  }
  return findings
}

function git(...args) {
  return execFileSync("git", args, { encoding: "utf8" }).trim()
}

export function changedFiles(base = "origin/main", cwd = process.cwd()) {
  validateBaseRef(base)
  const paths = (args) => execFileSync("git", args, { cwd }).toString("utf8").split("\0").filter(Boolean)
  const changed = paths(["diff", "--name-only", "--no-renames", "--diff-filter=ACMRDT", "-z", base])
  const staged = paths(["diff", "--cached", "--name-only", "--no-renames", "--diff-filter=ACMRDT", "-z", base])
  const unstaged = paths(["diff", "--name-only", "--no-renames", "--diff-filter=ACMRDT", "-z"])
  const untracked = execFileSync("git", ["ls-files", "--others", "--exclude-standard", "-z"], { cwd })
    .toString("utf8").split("\0").filter(Boolean)
  return [...new Set([...changed, ...staged, ...unstaged, ...untracked])]
}

function validateBaseRef(base) {
  if (!/^[A-Za-z0-9][A-Za-z0-9._/-]*$/.test(base) || base.includes("..")) throw new Error("Invalid base ref")
}

export function safeDiff(base = "origin/main", cwd = process.cwd()) {
  validateBaseRef(base)
  return execFileSync("git", [
    "diff", "--no-ext-diff", "--no-textconv", base, "--", ".",
    ":(exclude).env*", ":(exclude)**/.env*", ":(exclude)auth.json",
    ":(exclude)**/auth.json", ":(exclude)*.pem", ":(exclude)**/*.pem",
  ], { cwd, encoding: "utf8" })
}

function trackedFiles() {
  const output = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard", "-z"])
  return output.toString("utf8").split("\0").filter(Boolean)
}

function scanRepository() {
  const findings = []
  for (const file of trackedFiles()) {
    let bytes
    try { bytes = readFileSync(file) } catch { continue }
    if (bytes.length > 2_000_000 || bytes.includes(0)) continue
    for (const finding of scanSecretText(bytes.toString("utf8"))) findings.push({ file, ...finding })
  }
  return findings
}

function main() {
  const [command, arg] = process.argv.slice(2)
  if (command === "risk") {
    console.log(JSON.stringify(classifyFiles(changedFiles(arg)), null, 2))
  } else if (command === "diff") {
    process.stdout.write(safeDiff(arg))
  } else if (command === "secrets") {
    const findings = scanRepository()
    if (findings.length) {
      console.error(JSON.stringify({ findings }, null, 2))
      process.exitCode = 1
    } else console.log("No tracked-file secret patterns found.")
  } else if (command === "evidence") {
    const files = changedFiles(arg)
    console.log(JSON.stringify({
      commit: git("rev-parse", "HEAD"),
      branch: git("branch", "--show-current"),
      clean: git("status", "--porcelain") === "",
      ...classifyFiles(files),
      generated_at: new Date().toISOString(),
    }, null, 2))
  } else {
    console.error("Usage: node scripts/factory-check.mjs diff [BASE] | risk [BASE] | secrets | evidence [BASE]")
    process.exitCode = 1
  }
}

if (process.argv[1] && new URL(import.meta.url).pathname === process.argv[1]) main()
