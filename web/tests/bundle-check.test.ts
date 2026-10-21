import { expect, test } from "bun:test";
import { clientFiles, parseCsv, privateCells, secretsFromEnv } from "../../scripts/bundle-check";

test("client files: entry static imports and client screens with their imports, not other lazy chunks", () => {
  const manifest = {
    "index.html": { file: "a/index.js", isEntry: true, imports: ["_shared"], dynamicImports: ["staff.tsx", "home.tsx"], css: ["a/i.css"] },
    _shared: { file: "a/shared.js" },
    "home.tsx": { file: "a/home.js", imports: ["_shared"], dynamicImports: ["chart.tsx"] },
    "chart.tsx": { file: "a/chart.js" },
    "staff.tsx": { file: "a/staff.js" },
  };
  expect([...clientFiles(manifest, ["home.tsx"])].sort()).toEqual(["a/chart.js", "a/home.js", "a/i.css", "a/index.js", "a/shared.js"]);
  expect(() => clientFiles(manifest, ["gone.tsx"])).toThrow("not in the Vite manifest");
});

test("secrets from env and private sheet cells", () => {
  expect(secretsFromEnv("# c\nexport SOLANA_TRACKER_API_KEY='k-123'\nPUBLIC_RPC_URL=https://rpc\nFIRST_ADMIN_PASSWORD=\"pw\"\nNETWORK_SHEET_CSV_URL=\n")).toEqual(["k-123", "pw"]);
  expect(parseCsv('a,"b, c","d ""q"""\r\n1,2,3')).toEqual([["a", "b, c", 'd "q"'], ["1", "2", "3"]]);
  const csv = 'Name,Group,Low $,High $,Deliverable / note,FOMO,Wallet,\nalpha,"Direct, Misc","$1,000","$2,500",Post + buy,alpha,7xKXtg2CW87d97TXJSDpbD5jBkheTqA83TZRuJosgAsU,\nbeta,Direct,$300,-,,beta,broken link,\n';
  expect(privateCells(csv).sort()).toEqual(["$1,000", "$2,500", "Direct", "Direct, Misc", "Post + buy", "broken link"].sort());
});
