/**
 * 纯 Node.js 实现的 Minecraft RCON 客户端，无外部依赖。
 *
 * Source RCON 协议（小端序）：
 *   [int32 length][int32 request_id][int32 type][payload bytes][0x00][0x00]
 *   length = 4(request_id) + 4(type) + payload.length + 2(双 null 结尾)
 *
 * type:
 *   3 = SERVERDATA_AUTH            (登录请求)
 *   2 = SERVERDATA_EXECCOMMAND     (执行命令)
 *   2 = SERVERDATA_AUTH_RESPONSE   (登录响应，与命令同号)
 *   0 = SERVERDATA_RESPONSE_VALUE  (命令响应)
 *
 * 登录成功时，AUTH_RESPONSE 的 request_id 与请求一致；失败为 -1。
 */

import net from "node:net";

const SERVERDATA_AUTH = 3;
const SERVERDATA_EXECCOMMAND = 2;
const SERVERDATA_RESPONSE_VALUE = 0;

export class RconClient {
  /**
   * @param {object} opts
   * @param {string} opts.host
   * @param {number} opts.port
   * @param {string} opts.password
   */
  constructor({ host = "127.0.0.1", port = 25575, password }) {
    this.host = host;
    this.port = port;
    this.password = password;
    this.socket = null;
    this.nextId = 1;
    this.buffer = Buffer.alloc(0);
    this.packetListeners = [];
  }

  connect({ timeoutMs = 10000 } = {}) {
    return new Promise((resolve, reject) => {
      const socket = net.createConnection({ host: this.host, port: this.port });
      this.socket = socket;

      const onError = (err) => {
        cleanup();
        reject(err);
      };
      const onTimeout = () => {
        cleanup();
        socket.destroy();
        reject(new Error(`RCON 连接超时（${timeoutMs}ms）`));
      };
      const cleanup = () => {
        socket.removeListener("error", onError);
        clearTimeout(timer);
      };
      const timer = setTimeout(onTimeout, timeoutMs);

      socket.once("error", onError);
      socket.on("data", (chunk) => this._onData(chunk));
      socket.once("close", () => {
        for (const listener of [...this.packetListeners]) listener.reject?.(new Error("RCON 连接已关闭"));
      });
      socket.once("connect", async () => {
        cleanup();
        socket.on("error", (err) => {
          // 连接建立后的错误转交给挂起的监听器
          for (const l of [...this.packetListeners]) l.reject?.(err);
        });
        try {
          await this._authenticate(timeoutMs);
          resolve(this);
        } catch (err) {
          socket.destroy();
          reject(err);
        }
      });
    });
  }

  async _authenticate(timeoutMs) {
    const id = this._send(SERVERDATA_AUTH, this.password);
    const packet = await this._waitForPacket(
      (p) => p.type === SERVERDATA_EXECCOMMAND && (p.id === id || p.id === -1),
      timeoutMs,
    );
    // 部分服务端会先发一个空的 RESPONSE_VALUE，再发 AUTH_RESPONSE。
    // 上面的判定已放行 AUTH_RESPONSE；这里检查 id。
    if (packet.id === -1) {
      throw new Error("RCON 认证失败：密码错误");
    }
  }

  // Minecraft 的 RCON 实现可能把同一 TCP 读取中的两个请求当成坏包。
  // 每次只发一个命令，按请求 ID 收集分片，通过静默窗口结束响应。
  async send(command, { timeoutMs = 8000, settleMs = 200 } = {}) {
    const id = this._send(SERVERDATA_EXECCOMMAND, command);
    return new Promise((resolve, reject) => {
      let body = "";
      let settleTimer;
      const cleanup = () => {
        clearTimeout(hardTimer);
        clearTimeout(settleTimer);
        const index = this.packetListeners.indexOf(listener);
        if (index >= 0) this.packetListeners.splice(index, 1);
      };
      const listener = {
        onPacket: packet => {
          if (packet.id !== id) return;
          body += packet.body;
          clearTimeout(settleTimer);
          settleTimer = setTimeout(() => { cleanup(); resolve(body); }, settleMs);
        },
        reject: error => { cleanup(); reject(error); },
      };
      const hardTimer = setTimeout(() => listener.reject(new Error(`RCON 命令响应超时：${command}`)), timeoutMs);
      this.packetListeners.push(listener);
    });
  }

  close() {
    return new Promise((resolve) => {
      if (!this.socket || this.socket.destroyed) return resolve();
      const timer = setTimeout(() => { this.socket?.destroy(); resolve(); }, 2000);
      this.socket.once("close", () => { clearTimeout(timer); resolve(); });
      this.socket.end();

    });
  }

  // ---- 内部 ----

  _send(type, payload) {
    const id = this.nextId++;
    const payloadBuf = Buffer.from(payload, "utf8");
    const length = 4 + 4 + payloadBuf.length + 2;
    const packet = Buffer.alloc(4 + length);
    packet.writeInt32LE(length, 0);
    packet.writeInt32LE(id, 4);
    packet.writeInt32LE(type, 8);
    payloadBuf.copy(packet, 12);
    // 末尾两个 null 字节已由 alloc 置零
    if (!this.socket || this.socket.destroyed) throw new Error("RCON 连接不可用");
    this.socket.write(packet);
    return id;
  }

  _onData(chunk) {
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (this.buffer.length >= 4) {
      const length = this.buffer.readInt32LE(0);
      if (this.buffer.length < 4 + length) break; // 包未收全
      const id = this.buffer.readInt32LE(4);
      const type = this.buffer.readInt32LE(8);
      const bodyEnd = 4 + length - 2; // 去掉结尾两个 null
      const body = this.buffer.toString("utf8", 12, bodyEnd);
      this.buffer = this.buffer.subarray(4 + length);

      const packet = { id, type, body };
      // 派发给一次性 _waitForPacket 监听器与持续 send 监听器
      for (const l of [...this.packetListeners]) {
        l.onPacket?.(packet);
      }
    }
  }

  _waitForPacket(predicate, timeoutMs) {
    return new Promise((resolve, reject) => {
      const cleanup = () => {
        clearTimeout(timer);
        const index = this.packetListeners.indexOf(listener);
        if (index >= 0) this.packetListeners.splice(index, 1);
      };
      const listener = {
        onPacket: packet => { if (predicate(packet)) { cleanup(); resolve(packet); } },
        reject: error => { cleanup(); reject(error); },
      };
      const timer = setTimeout(() => listener.reject(new Error(`RCON 等待响应超时（${timeoutMs}ms）`)), timeoutMs);
      this.packetListeners.push(listener);
    });
  }

}
