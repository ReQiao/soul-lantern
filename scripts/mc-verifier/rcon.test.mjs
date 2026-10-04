import test from "node:test";
import assert from "node:assert/strict";
import net from "node:net";
import { RconClient } from "./rcon.mjs";

function packet(id, type, body = "") {
  const bytes = Buffer.from(body);
  const buffer = Buffer.alloc(bytes.length + 14);
  buffer.writeInt32LE(bytes.length + 10, 0);
  buffer.writeInt32LE(id, 4);
  buffer.writeInt32LE(type, 8);
  bytes.copy(buffer, 12);
  return buffer;
}

async function mockServer(t, respond) {
  const sockets = new Set();
  const server = net.createServer(socket => {
    sockets.add(socket);
    socket.on("close", () => sockets.delete(socket));
    let buffer = Buffer.alloc(0);
    socket.on("data", chunk => {
      buffer = Buffer.concat([buffer, chunk]);
      while (buffer.length >= 4 && buffer.length >= buffer.readInt32LE(0) + 4) {
        const length = buffer.readInt32LE(0);
        const request = { id: buffer.readInt32LE(4), type: buffer.readInt32LE(8), body: buffer.toString("utf8", 12, length + 2) };
        buffer = buffer.subarray(length + 4);
        respond(socket, request);
      }
    });
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  t.after(async () => {
    for (const socket of sockets) socket.destroy();
    await new Promise(resolve => server.close(resolve));
  });
  return new RconClient({ host: "127.0.0.1", port: server.address().port, password: "test" });
}

test("认证需等待类型 2，执行命令不发送额外围栏包，收集分片和空响应", async t => {
  let authenticated = false;
  const requests = [];
  const client = await mockServer(t, (socket, request) => {
    requests.push(request);
    if (request.type === 3) {
      socket.write(packet(request.id, 0));
      setTimeout(() => { authenticated = true; socket.write(packet(request.id, 2)); }, 25);
    } else if (request.body === "empty") {
      socket.write(packet(request.id, 0));
    } else {
      assert.ok(authenticated);
      const first = packet(request.id, 0, "第一段");
      socket.write(first.subarray(0, 7));
      setTimeout(() => socket.write(Buffer.concat([first.subarray(7), packet(request.id, 0, "第二段")])), 10);
    }
  });
  await client.connect();
  assert.ok(authenticated);
  assert.equal(await client.send("parts", { settleMs: 40 }), "第一段第二段");
  assert.equal(await client.send("empty", { settleMs: 40 }), "");
  assert.deepEqual(requests.map(request => request.type), [3, 2, 2]);
  await client.close();
});

test("密码错误不会被空认证确认包掩盖", async t => {
  const client = await mockServer(t, (socket, request) => {
    socket.write(Buffer.concat([packet(request.id, 0), packet(-1, 2)]));
  });
  await assert.rejects(client.connect(), /认证失败/);
});

test("连接中断立即失败，无响应明确超时", async t => {
  const client = await mockServer(t, (socket, request) => {
    if (request.type === 3) socket.write(packet(request.id, 2));
    else if (request.body === "close") socket.destroy();
  });
  await client.connect();
  await assert.rejects(client.send("timeout", { timeoutMs: 40 }), /响应超时/);
  await assert.rejects(client.send("close", { timeoutMs: 1000 }), /连接已关闭/);
  await assert.rejects(client.send("closed"), /连接不可用/);
});
