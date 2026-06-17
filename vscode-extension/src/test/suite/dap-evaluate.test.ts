// Tests for [PROFILE-MEMORY-COURIER]. See docs/specs/LSP-PROFILING-SPEC.md#PROFILE-MEMORY-COURIER
//
// debugpy truncates a single `print()` at ~20 KB, which silently corrupts the
// large JSON a real tracemalloc snapshot produces. The injection scripts now
// write their `__BASILISK_MEM*__ + json` payload to a temp file and print only
// `__BASILISK_MEM_FILE__<path>`; the editor reads the file back. These tests
// drive that resolver over real files (no mocks).

import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { resolveMarkerFilePayload } from "../../dap-evaluate";

suite("Memory courier — large payload file handoff", () => {
  test("a file-handoff marker is replaced by the file's full contents, then cleaned up", async () => {
    // 50 KB — comfortably past the ~20 KB stdout cap that truncated snapshots.
    // mkdtempSync gives a private, unpredictable dir (no insecure temp-file race).
    const payload = `__BASILISK_MEM__${JSON.stringify({ stats: "x".repeat(50_000) })}`;
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "basilisk-courier-"));
    const file = path.join(dir, "payload.txt");
    fs.writeFileSync(file, payload, "utf8");

    const resolved = await resolveMarkerFilePayload(`__BASILISK_MEM_FILE__${file}\n`);

    assert.strictEqual(resolved, payload, "the full payload must return intact from the file");
    assert.ok(!fs.existsSync(file), "the temp payload file must be removed after reading");
    fs.rmSync(dir, { recursive: true, force: true });
  });

  test("output without a file marker passes through unchanged (CPU acks, OK markers)", async () => {
    const direct = "__BASILISK_CPU_ACK__ok";
    assert.strictEqual(await resolveMarkerFilePayload(direct), direct);
  });

  test("a missing payload file falls back to the raw line instead of throwing", async () => {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "basilisk-courier-"));
    const missing = path.join(dir, "absent.txt");
    const raw = `__BASILISK_MEM_FILE__${missing}\n`;
    assert.strictEqual(await resolveMarkerFilePayload(raw), raw);
    fs.rmSync(dir, { recursive: true, force: true });
  });
});
