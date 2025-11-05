# vers-rs WASM demo

This demo shows how to use the vers-rs wasm bindings from a simple web page.

## Build steps

All commands should be run from the repository root.

1. Install wasm-pack if you don't have it:
   ```bash
   cargo install wasm-pack
   ```

2. Build the wasm package for the web (you might need to include `~/.cargo/bin/` in your `$PATH`):
   ```bash
   wasm-pack build --target web
   ```

   This produces a `pkg/` directory containing `vers_rs.js` and the wasm file.

3. Serve the `www/` folder from a static web server. For example:
   ```bash
   python3 -m http.server --directory www 8000
   ```

4. Open http://localhost:8000 in your browser.

## Notes
- The demo imports `../pkg/vers_rs.js`, so `pkg/` should be next to `www/` after building with wasm-pack.
- If you bundle differently (rollup/webpack/Vite), adapt the import path accordingly.
