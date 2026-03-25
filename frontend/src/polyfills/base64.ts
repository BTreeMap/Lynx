if (!Uint8Array.prototype.toBase64) {
  Object.defineProperty(Uint8Array.prototype, "toBase64", {
    value: function (this: Uint8Array, options: { alphabet?: 'base64' | 'base64url'; omitPadding?: boolean } = {}) {
      if (!(this instanceof Uint8Array)) throw new TypeError("Method Uint8Array.prototype.toBase64 called on incompatible receiver");
      if (typeof options !== "object" || options === null) throw new TypeError("The options argument must be an object");

      const alphabet = options.alphabet ?? "base64";
      const omitPadding = !!(options.omitPadding ?? false);

      if (alphabet !== "base64" && alphabet !== "base64url") throw new TypeError(`Invalid alphabet: ${alphabet}`);

      let binary = "";
      const len = this.length;
      const CHUNK_SIZE = 8192;
      for (let i = 0; i < len; i += CHUNK_SIZE) {
        binary += String.fromCharCode.apply(
          null,
          this.subarray(i, i + CHUNK_SIZE),
        );
      }

      let base64 = btoa(binary);
      if (alphabet === "base64url") {
        base64 = base64.replace(/\+/g, "-").replace(/\//g, "_");
      }
      if (omitPadding) {
        base64 = base64.replace(/=+$/, "");
      }
      return base64;
    },
    writable: true,
    configurable: true,
    enumerable: false,
  });
}
