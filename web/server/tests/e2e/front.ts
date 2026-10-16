import { createServer, request as forward, type Server } from "node:http";

/**
 * Orchester Cloud's front door in the run, as Caddy is in `deploy/cloud`: one public address that sends `/api/*` to
 * the backend and everything else to the web front, appending the client's address to `X-Forwarded-For`.
 */
export function startFront(port: number, api: number, web: number): Promise<Server> {
  const server = createServer((incoming, outgoing) => {
    const target = (incoming.url ?? "/").startsWith("/api/") ? api : web;
    const forwarded = [incoming.headers["x-forwarded-for"], incoming.socket.remoteAddress].filter(Boolean).join(", ");
    const upstream = forward(
      { host: "127.0.0.1", port: target, method: incoming.method, path: incoming.url,
        headers: { ...incoming.headers, "x-forwarded-for": forwarded } },
      (answer) => {
        outgoing.writeHead(answer.statusCode ?? 502, answer.headers);
        answer.pipe(outgoing);
      },
    );
    upstream.on("error", () => {
      outgoing.writeHead(502);
      outgoing.end();
    });
    incoming.pipe(upstream);
  });
  return new Promise((resolve) => server.listen(port, "::", () => { resolve(server); }));
}
