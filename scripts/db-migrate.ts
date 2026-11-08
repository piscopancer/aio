import { execFileSync } from 'node:child_process'
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'

const tauriDir = new URL('../src-tauri/', import.meta.url)
const schema = new URL('src/infrastructure/schema.rs', tauriDir)
/** Throwaway database, rebuilt from every migration so Diesel prints the full schema. */
const database = new URL('target/schema.sqlite', tauriDir)

mkdirSync(new URL('.', database), { recursive: true })
rmSync(database, { force: true })
execFileSync('diesel', ['migration', 'run', '--database-url', database.pathname.replace(/^\/(\w:)/, '$1')], {
  cwd: tauriDir,
  stdio: 'inherit',
})
// Diesel prints 4-space indentation, the project uses 2
const printed = readFileSync(schema, 'utf8')
writeFileSync(
  schema,
  printed.replace(/^(?: {4})+/gm, (indent) => ' '.repeat(indent.length / 2)),
)
