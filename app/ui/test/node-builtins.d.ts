// Hand-written ambient declarations for the small slice of Node's builtin
// modules the test suite touches. There is no @types/node in this project
// (zero new dependencies), so these exist only to let `tsc` see the shape
// of node:test, node:assert/strict, node:fs, node:path and node:url that
// the test files actually call. Not a general-purpose Node typing.

declare module "node:test" {
  type TestFn = () => void | Promise<void>;
  export default function test(name: string, fn: TestFn): void;
}

declare module "node:assert/strict" {
  interface Assert {
    (value: unknown, message?: string | Error): void;
    ok(value: unknown, message?: string | Error): void;
    equal(actual: unknown, expected: unknown, message?: string | Error): void;
    match(actual: string, expected: RegExp, message?: string | Error): void;
  }
  const assert: Assert;
  export default assert;
}

declare module "node:fs" {
  export function readFileSync(path: string, encoding: string): string;
}

declare module "node:path" {
  interface PathApi {
    dirname(p: string): string;
    join(...parts: string[]): string;
  }
  const path: PathApi;
  export default path;
}

declare module "node:url" {
  export function fileURLToPath(url: string | URL): string;
}
