import { expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { builtFiles, parseCsv, privateCells, secretsFromEnv } from "../../scripts/bundle-check";

test("built files: every JS and CSS file under dist, nested ones too", () => {
  const dir = mkdtempSync(join(tmpdir(), "bundle-"));
  try {
    mkdirSync(join(dir, "assets"));
    for (const f of ["index.html", "assets/a.js", "assets/Staff-1.js", "assets/a.css", "assets/f.woff2"]) writeFileSync(join(dir, f), "x");
    expect(builtFiles(dir)).toEqual(["assets/Staff-1.js", "assets/a.css", "assets/a.js"]);
  } finally {
    rmSync(dir, { recursive: true });
  }
});

test("secrets from env and private sheet cells", () => {
  expect(secretsFromEnv("# c\nexport SOLANA_TRACKER_API_KEY='k-123'\nPUBLIC_RPC_URL=https://rpc\nFIRST_ADMIN_PASSWORD=\"pw\"\nNETWORK_SHEET_CSV_URL=\n")).toEqual(["k-123", "pw"]);
  expect(parseCsv('a,"b, c","d ""q"""\r\n1,2,3')).toEqual([["a", "b, c", 'd "q"'], ["1", "2", "3"]]);
  const csv = 'Name,Group,Low $,High $,Deliverable / note,FOMO,Wallet,\nalpha,"Direct, Misc","$1,000","$2,500",Post + buy,alpha,7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU,\nbeta,Direct,$300,-,,beta,broken link,\n';
  expect(privateCells(csv).sort()).toEqual(["$1,000", "$2,500", "$300", "Direct", "Direct, Misc", "Post + buy", "broken link"].sort());
});
