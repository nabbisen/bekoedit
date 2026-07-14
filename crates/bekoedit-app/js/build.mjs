// Build the CodeMirror 6 editor bundle for bekoedit.
// Output: ../assets/editor-bundle.js  (IIFE, window.__bk)
// This artifact is committed so the app builds without Node at runtime.
import * as esbuild from "esbuild";

const watch = process.argv.includes("--watch");
const ctx = await esbuild.context({
  entryPoints: ["src/editor.js"],
  bundle: true,
  minify: !watch,
  sourcemap: watch ? "inline" : false,
  format: "iife",
  outfile: "../assets/editor-bundle.js",
  logLevel: "info",
});

if (watch) {
  await ctx.watch();
} else {
  await ctx.rebuild();
  await ctx.dispose();
}
