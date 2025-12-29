start-example:
	wasm-pack build && (cd example && deno install && deno run dev)
