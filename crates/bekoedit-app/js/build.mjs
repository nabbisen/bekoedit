// Build the committed browser bundles used by bekoedit at runtime.
import * as esbuild from "esbuild";

const watch = process.argv.includes("--watch");
const outputs = [
  ["src/focus-guard-bootstrap.js", "../assets/focus-guard-bundle.js"],
  ["src/editor.js", "../assets/editor-bundle.js"],
];
const contexts = await Promise.all(outputs.map(([entryPoint, outfile]) =>
  esbuild.context({
    entryPoints: [entryPoint],
    bundle: true,
    minify: !watch,
    sourcemap: watch ? "inline" : false,
    format: "iife",
    outfile,
    logLevel: "info",
  })));

if (watch) {
  await Promise.all(contexts.map((context) => context.watch()));
} else {
  try {
    await Promise.all(contexts.map((context) => context.rebuild()));
  } finally {
    await Promise.all(contexts.map((context) => context.dispose()));
  }
}
