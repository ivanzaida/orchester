import { createServer, type Server, type Socket } from "node:net";

/**
 * A minimal SMTP relay on loopback for the run's Orchester Cloud: it accepts every message and keeps it, decoded, so
 * the test can follow a confirmation link and see an invite arrive.
 */
export interface Mail {
  readonly to: string;
  readonly body: string;
}

export class SmtpSink {
  private readonly mails: Mail[] = [];
  private server: Server | undefined;

  async listen(port: number): Promise<void> {
    this.server = createServer((socket) => {
      this.session(socket);
    });
    const server = this.server;
    await new Promise<void>((resolve) => server.listen(port, "127.0.0.1", resolve));
  }

  close(): void {
    this.server?.close();
  }

  to(address: string): Mail[] {
    return this.mails.filter((mail) => mail.to === address.toLowerCase());
  }

  private session(socket: Socket): void {
    let buffer = "";
    let data: string | null = null;
    let recipient = "";
    socket.write("220 relay.test ESMTP\r\n");
    socket.on("data", (chunk: Buffer) => {
      buffer += chunk.toString("utf8");
      let end = buffer.indexOf("\r\n");
      while (end >= 0) {
        const line = buffer.slice(0, end);
        buffer = buffer.slice(end + 2);
        if (data !== null) {
          if (line === ".") {
            this.mails.push({ to: recipient, body: decode(data) });
            data = null;
            socket.write("250 queued\r\n");
          } else {
            data += `${line.startsWith("..") ? line.slice(1) : line}\n`;
          }
        } else {
          const reply = command(line);
          if (reply.recipient !== undefined) {
            recipient = reply.recipient;
          }
          if (reply.data) {
            data = "";
          }
          socket.write(reply.text);
          if (reply.quit) {
            socket.end();
          }
        }
        end = buffer.indexOf("\r\n");
      }
    });
  }
}

interface Reply {
  readonly text: string;
  readonly recipient?: string;
  readonly data?: boolean;
  readonly quit?: boolean;
}

function command(line: string): Reply {
  const upper = line.toUpperCase();
  if (upper.startsWith("EHLO") || upper.startsWith("HELO")) {
    return { text: "250-relay.test\r\n250 8BITMIME\r\n" };
  }
  if (upper.startsWith("RCPT TO:")) {
    const address = /<([^>]+)>/.exec(line)?.[1] ?? "";
    return { text: "250 ok\r\n", recipient: address.toLowerCase() };
  }
  if (upper.startsWith("DATA")) {
    return { text: "354 go ahead\r\n", data: true };
  }
  if (upper.startsWith("QUIT")) {
    return { text: "221 bye\r\n", quit: true };
  }
  return { text: "250 ok\r\n" };
}

/** The message's body, undoing its transfer encoding (quoted-printable or base64), so its links read whole. */
function decode(message: string): string {
  const split = message.indexOf("\n\n");
  const head = message.slice(0, split).toLowerCase();
  const body = message.slice(split + 2);
  if (head.includes("content-transfer-encoding: base64")) {
    return Buffer.from(body.replace(/\s+/g, ""), "base64").toString("utf8");
  }
  if (head.includes("content-transfer-encoding: quoted-printable")) {
    const joined = body.replace(/=\n/g, "");
    const bytes = joined.replace(/=([0-9A-F]{2})/g, (_, hex: string) => String.fromCharCode(parseInt(hex, 16)));
    return Buffer.from(bytes, "latin1").toString("utf8");
  }
  return body;
}
