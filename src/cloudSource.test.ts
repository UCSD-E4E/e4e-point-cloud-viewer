import { describe, expect, it } from "vitest";
import {
  displayNameFromUrl,
  loadCloudFromFile,
  loadCloudFromUrl,
  parseCloudParam,
} from "./cloudSource.ts";

/** A packed `.e4epc` buffer with 2 splats — mirrors the splatCloud / Rust fixture. */
function packedCloud(): ArrayBuffer {
  const buf = new ArrayBuffer(4 + 24 + 8 + 24 + 8);
  new DataView(buf).setUint32(0, 2, true);
  new Float32Array(buf, 4, 6).set([1, 2, 3, 4, 5, 6]);
  new Uint8Array(buf, 28, 8).set([10, 20, 30, 40, 50, 60, 70, 80]);
  new Float32Array(buf, 36, 6).set([0, 0, 1, 0, 1, 0]);
  new Float32Array(buf, 60, 2).set([0.5, 0.7]);
  return buf;
}

describe("parseCloudParam", () => {
  it("returns the cloud param when present", () => {
    expect(parseCloudParam("?cloud=/fixtures/autzen.e4epc")).toBe("/fixtures/autzen.e4epc");
  });

  it("returns null when there is no cloud param", () => {
    expect(parseCloudParam("")).toBeNull();
    expect(parseCloudParam("?foo=bar")).toBeNull();
  });

  it("returns null for an empty cloud value", () => {
    expect(parseCloudParam("?cloud=")).toBeNull();
  });

  it("percent-decodes the value", () => {
    expect(parseCloudParam("?cloud=%2Fa%20b.e4epc")).toBe("/a b.e4epc");
  });
});

describe("displayNameFromUrl", () => {
  it("takes the last path segment", () => {
    expect(displayNameFromUrl("/fixtures/autzen.e4epc")).toBe("autzen.e4epc");
    expect(displayNameFromUrl("https://host.example/c/abc123.e4epc")).toBe("abc123.e4epc");
  });

  it("strips a trailing query string and hash", () => {
    expect(displayNameFromUrl("/c/x.e4epc?v=2#frag")).toBe("x.e4epc");
  });

  it("percent-decodes the segment", () => {
    expect(displayNameFromUrl("/c/my%20scan.e4epc")).toBe("my scan.e4epc");
  });

  it("falls back to the input when there is no usable segment", () => {
    expect(displayNameFromUrl("")).toBe("");
  });
});

describe("loadCloudFromUrl", () => {
  it("fetches the blob, decodes it, and labels it from the URL", async () => {
    const fetchStub = async (url: string) => {
      expect(url).toBe("/fixtures/autzen.e4epc");
      return {
        ok: true,
        status: 200,
        statusText: "OK",
        arrayBuffer: async () => packedCloud(),
      };
    };
    const { cloud, name } = await loadCloudFromUrl("/fixtures/autzen.e4epc", fetchStub);
    expect(cloud.count).toBe(2);
    expect(Array.from(cloud.positions)).toEqual([1, 2, 3, 4, 5, 6]);
    expect(name).toBe("autzen.e4epc");
  });

  it("rejects when the response is not ok", async () => {
    const fetchStub = async () => ({
      ok: false,
      status: 404,
      statusText: "Not Found",
      arrayBuffer: async () => new ArrayBuffer(0),
    });
    await expect(loadCloudFromUrl("/missing.e4epc", fetchStub)).rejects.toThrow(/404/);
  });
});

describe("loadCloudFromFile", () => {
  it("decodes a File and uses its name", async () => {
    const file = new File([packedCloud()], "scan.e4epc");
    const { cloud, name } = await loadCloudFromFile(file);
    expect(cloud.count).toBe(2);
    expect(Array.from(cloud.positions)).toEqual([1, 2, 3, 4, 5, 6]);
    expect(name).toBe("scan.e4epc");
  });

  it("decodes a plain Blob that has no name", async () => {
    const blob = new Blob([packedCloud()]);
    const { cloud } = await loadCloudFromFile(blob);
    expect(cloud.count).toBe(2);
  });
});
