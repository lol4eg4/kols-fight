import "@arena/design/styles.css";
import "./shell/shell.css";
import { mountApp } from "./shell";
import { plugins } from "./plugins.gen";

mountApp(document.getElementById("root")!, plugins);
