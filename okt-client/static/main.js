// Loads the client (Rust compiled to WebAssembly; see okt-client/src).
import init from './okt.js';
init({ module_or_path: '/okt_bg.wasm' });
