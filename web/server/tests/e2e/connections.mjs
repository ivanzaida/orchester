// Loaded into the admin app with `node --import` by the end-to-end run: records the host and port of every TCP
// connection the admin's server side opens, so the air-gapped run can show it reached only the team server.
import { appendFileSync } from "node:fs";
import net from "node:net";
import process from "node:process";

const file = process.env.SA_E2E_CONNECTIONS;

if (file !== undefined && file !== "") {
  const connect = net.Socket.prototype.connect;
  net.Socket.prototype.connect = function recorded(...args) {
    // `net.connect` hands its parsed arguments on as one array, `[options, callback]`.
    const [first, second] = Array.isArray(args[0]) ? args[0] : args;
    const options = typeof first === "object" && first !== null ? first : { port: first, host: second };
    if (options.path === undefined) {
      appendFileSync(file, `${String(options.host ?? "localhost")}:${String(options.port)}\n`);
    }
    return connect.apply(this, args);
  };
}
