import { lazy } from "react";
import { definePlugin } from "@arena/shell";
import { dict } from "./en";

export const plugin = definePlugin({
  name: "wallets",
  screens: { wallets: () => import("./Wallets") },
  // staff only: loaded with the Network screen, never in the client bundle
  fills: { "roster/wallet_status": lazy(() => import("./WalletStatus")) },
  widgets: {},
  dict,
});

export default plugin;
