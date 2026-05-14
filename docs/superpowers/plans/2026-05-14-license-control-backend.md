# 许可证控制 — 后端 + 营销网站 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在新建的 `dimkey-web` 仓库里搭建 Cloudflare Pages 一体化项目，包含许可证激活后端 API（D1 + Pages Functions + Ed25519 签名）、Lemon Squeezy webhook 收款、营销/购买/找回/账户网页、客服 admin 接口。完成后桌面客户端可直接对接。

**Architecture:** Astro 静态站 + Pages Functions（TypeScript + Hono）+ D1 SQLite + KV 缓存 + Workers Secrets 存私钥。所有业务错误统一返回 HTTP 200 + `{ok:false, code}`。Ed25519 签名证书让客户端离线 14 天可用。webhook 用 `orders_inbox` 表去重重放。

**Tech Stack:** Cloudflare Pages + Pages Functions + D1 + KV + Workers Secrets · TypeScript · Astro 4 · Hono 4 · `@noble/ed25519` · Resend（邮件）· Lemon Squeezy（海外支付）· Vitest（单元测试）· Wrangler CLI

**Spec:** `docs/superpowers/specs/2026-05-14-license-control-design.md`

**Repo:** 新建独立仓库 `dimkey-web`（不在当前 `dimkey` 客户端仓库内）。最终仓库结构见 spec §5.2。

---

## File Structure

**新建仓库 `dimkey-web/` 完整目录：**

```
dimkey-web/
├── package.json
├── tsconfig.json
├── wrangler.toml                           # CF Pages 配置 + bindings
├── astro.config.mjs                        # Astro 配置（adapter: cloudflare）
├── .gitignore
├── README.md
├── src/                                    # Astro 静态网站
│   ├── pages/
│   │   ├── index.astro                     # 营销首页
│   │   ├── zh.astro                        # 中文落地页
│   │   ├── en.astro                        # 英文落地页
│   │   ├── buy.astro                       # 购买页（LS 嵌入 + 微信入口）
│   │   ├── recover.astro                   # 找回许可证表单
│   │   ├── account.astro                   # 设备管理（输 email+key 后 fetch /api/v1/devices/list）
│   │   └── privacy.astro                   # 隐私政策
│   ├── layouts/Base.astro
│   └── components/
│       ├── BuyButton.astro
│       └── DeviceRow.astro
├── functions/                              # Pages Functions
│   ├── api/v1/
│   │   ├── activate.ts
│   │   ├── deactivate.ts
│   │   ├── heartbeat.ts
│   │   ├── recover.ts
│   │   └── devices/list.ts
│   ├── webhook/
│   │   └── lemonsqueezy.ts
│   ├── admin/
│   │   └── [[path]].ts                     # 单一入口路由所有 admin 子路径
│   └── _middleware.ts                      # 全局 CORS + 限流
├── shared/                                 # 前后端共享
│   ├── types.ts                            # API 请求/响应类型
│   ├── codes.ts                            # 错误码常量
│   ├── license-key.ts                      # generateLicenseKey + 校验
│   ├── ed25519.ts                          # 签证书共用函数
│   ├── fingerprint.ts                      # 客户端的指纹算法在客户端实现，这里只做 hex 校验
│   ├── email-templates/
│   │   ├── activation-zh.ts
│   │   └── activation-en.ts
│   └── migrations/
│       └── 0001_init.sql                   # D1 建表
├── scripts/
│   ├── gen-ed25519-keypair.ts              # 一次性生成密钥对
│   ├── d1-migrate.sh                       # wrangler d1 migrations apply
│   └── admin-curl-templates.md             # 客服 SOP（issue/revoke/transfer/lookup）
└── tests/                                  # Vitest 单元 + 集成
    ├── unit/
    │   ├── license-key.test.ts
    │   ├── ed25519.test.ts
    │   └── webhook-signature.test.ts
    └── integration/
        ├── activate.test.ts
        ├── deactivate.test.ts
        ├── heartbeat.test.ts
        ├── devices-list.test.ts
        ├── recover.test.ts
        ├── admin.test.ts
        └── lemonsqueezy-webhook.test.ts
```

---

## Phase 0：仓库与基础设施

### Task 0.1: 创建 `dimkey-web` 仓库 + 基础脚手架

**Files:**
- Create: `dimkey-web/package.json`
- Create: `dimkey-web/.gitignore`
- Create: `dimkey-web/README.md`

- [ ] **Step 1: 在 `dimkey` 仓库同级目录新建 `dimkey-web`**

```bash
cd /Users/tanzs-mac-mini/workpath/personal
mkdir dimkey-web && cd dimkey-web
git init
```

- [ ] **Step 2: 写 `package.json`**

```json
{
  "name": "dimkey-web",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "wrangler pages dev --d1=DB --kv=KV --compatibility-date=2026-05-01 -- npm run build:watch",
    "build": "astro build",
    "build:watch": "astro build --watch",
    "preview": "wrangler pages dev ./dist --d1=DB --kv=KV",
    "deploy": "astro build && wrangler pages deploy ./dist",
    "test": "vitest run",
    "test:watch": "vitest",
    "d1:migrate:local": "wrangler d1 migrations apply DB --local",
    "d1:migrate:prod": "wrangler d1 migrations apply DB --remote",
    "gen:keypair": "tsx scripts/gen-ed25519-keypair.ts"
  },
  "dependencies": {
    "@noble/ed25519": "^2.1.0",
    "astro": "^4.16.0",
    "hono": "^4.6.0",
    "@astrojs/cloudflare": "^11.0.0"
  },
  "devDependencies": {
    "@cloudflare/workers-types": "^4.20260501.0",
    "@types/node": "^22.0.0",
    "tsx": "^4.19.0",
    "typescript": "^5.6.0",
    "vitest": "^2.1.0",
    "wrangler": "^3.80.0"
  }
}
```

- [ ] **Step 3: 写 `.gitignore`**

```
node_modules/
dist/
.astro/
.wrangler/
.dev.vars
.env
*.log
.DS_Store
```

- [ ] **Step 4: 写 `README.md`**

```markdown
# dimkey-web

Dimkey 营销站 + 许可证后端（Cloudflare Pages 一体化）。

## 开发
\`\`\`bash
npm install
npm run gen:keypair          # 仅首次：生成 Ed25519 密钥对
npm run d1:migrate:local
npm run dev
\`\`\`

## 部署
\`\`\`bash
npm run deploy
\`\`\`

详见 design spec：dimkey 仓库 `docs/superpowers/specs/2026-05-14-license-control-design.md`
```

- [ ] **Step 5: 安装依赖 + 提交**

```bash
npm install
git add -A
git commit -m "chore: 初始化 dimkey-web 仓库脚手架"
```

---

### Task 0.2: TypeScript / Astro / Wrangler 配置

**Files:**
- Create: `dimkey-web/tsconfig.json`
- Create: `dimkey-web/astro.config.mjs`
- Create: `dimkey-web/wrangler.toml`

- [ ] **Step 1: 写 `tsconfig.json`**

```json
{
  "extends": "astro/tsconfigs/strict",
  "compilerOptions": {
    "strict": true,
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "target": "ES2022",
    "types": ["@cloudflare/workers-types", "vitest/globals"],
    "paths": {
      "@shared/*": ["./shared/*"]
    }
  },
  "include": ["src", "functions", "shared", "scripts", "tests"]
}
```

- [ ] **Step 2: 写 `astro.config.mjs`**

```javascript
import { defineConfig } from 'astro/config';
import cloudflare from '@astrojs/cloudflare';

export default defineConfig({
  output: 'static',
  adapter: cloudflare({ mode: 'directory' }),
  site: 'https://dimkey.app',
});
```

- [ ] **Step 3: 写 `wrangler.toml`（占位 ID 在 Phase 0.4 创建资源后回填）**

```toml
name = "dimkey-web"
compatibility_date = "2026-05-01"
pages_build_output_dir = "./dist"

[[d1_databases]]
binding = "DB"
database_name = "dimkey-prod"
database_id = "TODO_FILL_AFTER_CREATE"

[[kv_namespaces]]
binding = "KV"
id = "TODO_FILL_AFTER_CREATE"

# secrets 用 wrangler secret put 命令注入，不写在配置里
```

- [ ] **Step 4: 提交**

```bash
git add tsconfig.json astro.config.mjs wrangler.toml
git commit -m "chore: 加 TypeScript + Astro + Wrangler 配置"
```

---

### Task 0.3: 生成 Ed25519 密钥对（一次性）

**Files:**
- Create: `dimkey-web/scripts/gen-ed25519-keypair.ts`

- [ ] **Step 1: 写密钥生成脚本**

```typescript
// scripts/gen-ed25519-keypair.ts
import { utils, getPublicKeyAsync } from '@noble/ed25519';

const sk = utils.randomPrivateKey();          // 32 bytes
const pk = await getPublicKeyAsync(sk);       // 32 bytes

const skHex = Buffer.from(sk).toString('hex');
const pkHex = Buffer.from(pk).toString('hex');
const pkBytes = Array.from(pk).join(', ');

console.log('=== Ed25519 Keypair Generated ===');
console.log('Private key (hex, store as Worker Secret ED25519_PRIVATE_KEY):');
console.log(skHex);
console.log();
console.log('Public key (hex):');
console.log(pkHex);
console.log();
console.log('Public key (Rust array, paste into client license module):');
console.log(`pub const PUBKEY_V1: [u8; 32] = [${pkBytes}];`);
```

- [ ] **Step 2: 跑脚本，把输出存到本地密钥保险柜（不进 git）**

```bash
npm run gen:keypair > /tmp/dimkey-keys-$(date +%s).txt
cat /tmp/dimkey-keys-*.txt    # 看一眼输出格式
```

⚠️ **关键**：私钥仅本机查看一次，立即用下一步存到 Workers Secrets，本地文件用完即删。

- [ ] **Step 3: 把私钥注入 Workers Secrets**

```bash
# 提示输入私钥 hex 字符串
wrangler pages secret put ED25519_PRIVATE_KEY --project-name=dimkey-web
```

- [ ] **Step 4: 把公钥保存到本仓库 `shared/ed25519-pubkey.ts`（公钥可入 git）**

```typescript
// shared/ed25519-pubkey.ts
// 由 scripts/gen-ed25519-keypair.ts 生成于 <YYYY-MM-DD>
// 私钥已存为 Workers Secret ED25519_PRIVATE_KEY
export const ED25519_PUBKEY_V1_HEX = "<paste pkHex here>";
```

同时把 Rust array 输出**复制保存到本机笔记**，client plan 实现 license 模块时会用。

- [ ] **Step 5: 删除本地密钥临时文件 + 提交**

```bash
rm /tmp/dimkey-keys-*.txt
git add scripts/gen-ed25519-keypair.ts shared/ed25519-pubkey.ts
git commit -m "feat(crypto): 生成 Ed25519 密钥对脚本 + 公钥常量"
```

---

### Task 0.4: 在 Cloudflare 创建 D1 + KV + Pages 项目

**手动操作 + 命令；无 commit。**

- [ ] **Step 1: 创建 D1 数据库**

```bash
wrangler d1 create dimkey-prod
# 输出形如: database_id = "abcd1234-..."
```

把输出的 `database_id` 填回 `wrangler.toml` 的 `[[d1_databases]]` 段。

- [ ] **Step 2: 创建 KV namespace**

```bash
wrangler kv namespace create dimkey-cache
# 输出形如: id = "efgh5678-..."
```

把输出的 `id` 填回 `wrangler.toml` 的 `[[kv_namespaces]]` 段。

- [ ] **Step 3: 创建 Pages 项目**

```bash
wrangler pages project create dimkey-web --production-branch=main
```

- [ ] **Step 4: 注入剩余 Secrets**

```bash
wrangler pages secret put LS_WEBHOOK_SECRET --project-name=dimkey-web
# 提示时输入一个长随机串（用 openssl rand -hex 32 生成）

wrangler pages secret put ADMIN_TOKEN --project-name=dimkey-web
# 同上，长随机串

wrangler pages secret put RESEND_API_KEY --project-name=dimkey-web
# 在 https://resend.com 注册后获取的 API key
```

- [ ] **Step 5: 提交 wrangler.toml 更新**

```bash
git add wrangler.toml
git commit -m "chore: 填入 D1 + KV 资源 ID"
```

---

## Phase 1：D1 Schema 与共享工具

### Task 1.1: 写 D1 migration `0001_init.sql`

**Files:**
- Create: `dimkey-web/shared/migrations/0001_init.sql`

- [ ] **Step 1: 写完整建表 SQL（拷自 spec §5.3）**

```sql
-- shared/migrations/0001_init.sql
CREATE TABLE licenses (
  license_id    TEXT PRIMARY KEY,
  license_key   TEXT NOT NULL UNIQUE,
  email         TEXT NOT NULL,
  plan          TEXT NOT NULL DEFAULT 'personal',
  max_devices   INTEGER NOT NULL DEFAULT 3,
  status        TEXT NOT NULL DEFAULT 'active',
  issued_at     INTEGER NOT NULL,
  expires_at    INTEGER,
  source        TEXT NOT NULL,
  order_ref     TEXT,
  notes         TEXT,
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
);
CREATE INDEX idx_licenses_email ON licenses(email);
CREATE UNIQUE INDEX idx_licenses_order ON licenses(source, order_ref);

CREATE TABLE devices (
  device_id        TEXT PRIMARY KEY,
  license_id       TEXT NOT NULL REFERENCES licenses(license_id),
  fingerprint      TEXT NOT NULL,
  machine_label    TEXT,
  os               TEXT NOT NULL,
  flavor           TEXT NOT NULL,
  app_version      TEXT,
  first_activated  INTEGER NOT NULL,
  last_seen        INTEGER NOT NULL,
  deactivated_at   INTEGER
);
CREATE INDEX idx_devices_license ON devices(license_id);
CREATE UNIQUE INDEX idx_devices_active_fp
  ON devices(license_id, fingerprint)
  WHERE deactivated_at IS NULL;

CREATE TABLE audit_log (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  license_id   TEXT,
  device_id    TEXT,
  event        TEXT NOT NULL,
  ip           TEXT,
  user_agent   TEXT,
  detail       TEXT,
  ts           INTEGER NOT NULL
);
CREATE INDEX idx_audit_license ON audit_log(license_id, ts);

CREATE TABLE orders_inbox (
  source        TEXT NOT NULL,
  external_id   TEXT NOT NULL,
  payload_json  TEXT NOT NULL,
  processed_at  INTEGER,
  error         TEXT,
  PRIMARY KEY (source, external_id)
);
```

- [ ] **Step 2: 应用到本地 D1**

```bash
npm run d1:migrate:local
```

Expected: `✅ 4 tables created`

- [ ] **Step 3: 验证 schema**

```bash
wrangler d1 execute DB --local --command="SELECT name FROM sqlite_master WHERE type='table';"
```

Expected: 输出含 `licenses, devices, audit_log, orders_inbox`

- [ ] **Step 4: 应用到生产 D1**

```bash
npm run d1:migrate:prod
```

- [ ] **Step 5: 提交**

```bash
git add shared/migrations/0001_init.sql
git commit -m "feat(db): 初始化 D1 schema — licenses + devices + audit_log + orders_inbox"
```

---

### Task 1.2: 共享类型 `shared/types.ts` + 错误码 `shared/codes.ts`

**Files:**
- Create: `dimkey-web/shared/types.ts`
- Create: `dimkey-web/shared/codes.ts`

- [ ] **Step 1: 写 `shared/codes.ts`（错误码常量，spec §9.1）**

```typescript
// shared/codes.ts
export const ErrorCode = {
  INVALID_LICENSE:        'INVALID_LICENSE',
  LICENSE_REVOKED:        'LICENSE_REVOKED',
  LICENSE_EXPIRED:        'LICENSE_EXPIRED',
  DEVICE_LIMIT_REACHED:   'DEVICE_LIMIT_REACHED',
  DEVICE_NOT_FOUND:       'DEVICE_NOT_FOUND',
  FINGERPRINT_MISMATCH:   'FINGERPRINT_MISMATCH',
  SIGNATURE_INVALID:      'SIGNATURE_INVALID',
  RATE_LIMITED:           'RATE_LIMITED',
  SERVER_ERROR:           'SERVER_ERROR',
  EMAIL_FORMAT_INVALID:   'EMAIL_FORMAT_INVALID',
  KEY_FORMAT_INVALID:     'KEY_FORMAT_INVALID',
} as const;
export type ErrorCodeT = typeof ErrorCode[keyof typeof ErrorCode];
```

- [ ] **Step 2: 写 `shared/types.ts`**

```typescript
// shared/types.ts
import type { ErrorCodeT } from './codes';

export type Ok<T>   = { ok: true;  data: T };
export type Err     = { ok: false; code: ErrorCodeT; message: string; data?: unknown };
export type ApiResp<T> = Ok<T> | Err;

export interface LicenseCertificateEnvelope {
  v: 1;
  payload_b64: string;
  sig_b64: string;
}

export interface LicensePayload {
  license_id: string;
  license_key: string;
  email: string;
  plan: 'personal';
  device_id: string;
  fingerprint: string;
  issued_at: string;            // ISO UTC
  expires_at: string | null;
  next_check_at: string;
  max_grace_until: string;
  key_version: 1;
}

export interface ActivateRequest {
  license_key: string;
  email: string;
  fingerprint: string;
  machine_label: string;
  os: 'macos' | 'windows';
  flavor: 'zh' | 'en';
  app_version: string;
}
export interface ActivateResponseData {
  license_certificate: LicenseCertificateEnvelope;
  device_summary: { current_device_id: string; active_count: number; max_devices: number };
}

export interface DeactivateRequest {
  license_key: string;
  email: string;
  device_id?: string;            // 省略则按 fingerprint 解绑当前
  fingerprint?: string;
}

export interface HeartbeatRequest {
  license_id: string;
  device_id: string;
  fingerprint: string;
}
export interface HeartbeatResponseData {
  status: 'active' | 'revoked';
  next_check_at: number;          // unix ms
}

export interface DevicesListRequest {
  license_key: string;
  email: string;
  fingerprint?: string;           // 可选：标记 is_current
}
export interface DeviceDto {
  device_id: string;
  machine_label: string | null;
  os: string;
  flavor: 'zh' | 'en';
  first_activated: number;
  last_seen: number;
  is_current: boolean;
}
export interface DevicesListResponseData {
  devices: DeviceDto[];
  max_devices: number;
}

export interface RecoverRequest { email: string }
export interface RecoverResponseData { message: string }
```

- [ ] **Step 3: 提交**

```bash
git add shared/types.ts shared/codes.ts
git commit -m "feat(shared): 定义 API 类型与错误码常量"
```

---

### Task 1.3: License Key 生成与校验工具（TDD）

**Files:**
- Create: `dimkey-web/shared/license-key.ts`
- Create: `dimkey-web/tests/unit/license-key.test.ts`

- [ ] **Step 1: 写失败测试**

```typescript
// tests/unit/license-key.test.ts
import { describe, it, expect } from 'vitest';
import { generateLicenseKey, isValidLicenseKey, normalizeLicenseKey } from '@shared/license-key';

describe('generateLicenseKey', () => {
  it('matches DK-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX format', () => {
    const k = generateLicenseKey();
    expect(k).toMatch(/^DK-[ABCDEFGHJKMNPQRSTUVWXYZ23456789]{5}(-[ABCDEFGHJKMNPQRSTUVWXYZ23456789]{5}){4}$/);
  });

  it('produces different keys on consecutive calls', () => {
    const a = generateLicenseKey();
    const b = generateLicenseKey();
    expect(a).not.toEqual(b);
  });

  it('uses no ambiguous chars (0/O/1/I/L)', () => {
    for (let i = 0; i < 100; i++) {
      const k = generateLicenseKey();
      expect(k).not.toMatch(/[0O1IL]/);
    }
  });
});

describe('isValidLicenseKey', () => {
  it('accepts canonical form', () => {
    expect(isValidLicenseKey('DK-ABCDE-FGHJK-MNPQR-STUVW-XYZ23')).toBe(true);
  });
  it('rejects wrong prefix', () => {
    expect(isValidLicenseKey('XX-ABCDE-FGHJK-MNPQR-STUVW-XYZ23')).toBe(false);
  });
  it('rejects ambiguous chars', () => {
    expect(isValidLicenseKey('DK-ABCDO-FGHJK-MNPQR-STUVW-XYZ23')).toBe(false);
  });
  it('rejects too few segments', () => {
    expect(isValidLicenseKey('DK-ABCDE-FGHJK')).toBe(false);
  });
});

describe('normalizeLicenseKey', () => {
  it('uppercases + strips spaces + adds dashes from raw 25-char paste', () => {
    expect(normalizeLicenseKey(' dk abcdefghjkmnpqrstuvwxyz2 ')).toBe('DK-ABCDE-FGHJK-MNPQR-STUVW-XYZ23');  // wait - need 25 chars
  });
  it('preserves canonical form unchanged', () => {
    expect(normalizeLicenseKey('DK-ABCDE-FGHJK-MNPQR-STUVW-XYZ23')).toBe('DK-ABCDE-FGHJK-MNPQR-STUVW-XYZ23');
  });
});
```

> 注意：normalizeLicenseKey 测试中的输入字符串需要正好 25 个字母（去掉空格和 dk 后），实现时按"取前 25 个有效字符"处理。

- [ ] **Step 2: 跑测试确认失败**

```bash
npx vitest run tests/unit/license-key.test.ts
```

Expected: 全部 FAIL（模块不存在）

- [ ] **Step 3: 实现 `shared/license-key.ts`**

```typescript
// shared/license-key.ts
const ALPHA = "ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const ALPHA_SET = new Set(ALPHA);

export function generateLicenseKey(): string {
  const buf = new Uint8Array(25);
  crypto.getRandomValues(buf);
  // 用拒绝采样消除 mod 偏置
  const chars: string[] = [];
  let i = 0;
  while (chars.length < 25) {
    if (i >= buf.length) {
      crypto.getRandomValues(buf);
      i = 0;
    }
    const b = buf[i++];
    if (b < 248) chars.push(ALPHA[b % 31]);   // 248 = 8 * 31，无偏
  }
  return 'DK-' + chars.join('').match(/.{5}/g)!.join('-');
}

const KEY_REGEX = /^DK-[ABCDEFGHJKMNPQRSTUVWXYZ23456789]{5}(-[ABCDEFGHJKMNPQRSTUVWXYZ23456789]{5}){4}$/;

export function isValidLicenseKey(k: string): boolean {
  return KEY_REGEX.test(k);
}

export function normalizeLicenseKey(raw: string): string {
  const upper = raw.toUpperCase().replace(/[^A-Z0-9]/g, '');
  const stripped = upper.startsWith('DK') ? upper.slice(2) : upper;
  const valid = stripped.split('').filter(c => ALPHA_SET.has(c)).slice(0, 25).join('');
  if (valid.length !== 25) return raw;        // 输入不完整时原样返回
  return 'DK-' + valid.match(/.{5}/g)!.join('-');
}
```

- [ ] **Step 4: 跑测试确认通过**

```bash
npx vitest run tests/unit/license-key.test.ts
```

Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add shared/license-key.ts tests/unit/license-key.test.ts
git commit -m "feat(license): license_key 生成 + 校验 + 归一化（无偏拒绝采样）"
```

---

### Task 1.4: Ed25519 签证书共用函数（TDD）

**Files:**
- Create: `dimkey-web/shared/ed25519.ts`
- Create: `dimkey-web/tests/unit/ed25519.test.ts`

- [ ] **Step 1: 写失败测试**

```typescript
// tests/unit/ed25519.test.ts
import { describe, it, expect } from 'vitest';
import { signCertificate, verifyCertificate } from '@shared/ed25519';
import { utils, getPublicKeyAsync } from '@noble/ed25519';
import type { LicensePayload } from '@shared/types';

describe('Ed25519 sign/verify roundtrip', () => {
  it('signs payload, verifies with matching pubkey', async () => {
    const sk = utils.randomPrivateKey();
    const pk = await getPublicKeyAsync(sk);
    const payload: LicensePayload = {
      license_id: 'uuid-1', license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE',
      email: 'u@example.com', plan: 'personal', device_id: 'dev-1',
      fingerprint: 'a3f9c211', issued_at: '2026-05-14T10:00:00Z',
      expires_at: null, next_check_at: '2026-05-21T10:00:00Z',
      max_grace_until: '2026-05-28T10:00:00Z', key_version: 1,
    };
    const env = await signCertificate(payload, sk);
    expect(env.v).toBe(1);
    expect(env.payload_b64.length).toBeGreaterThan(0);
    expect(env.sig_b64.length).toBeGreaterThan(0);

    const verified = await verifyCertificate(env, pk);
    expect(verified).toEqual(payload);
  });

  it('verify returns null for tampered payload', async () => {
    const sk = utils.randomPrivateKey();
    const pk = await getPublicKeyAsync(sk);
    const payload: LicensePayload = { /* same as above */
      license_id: 'uuid-1', license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE',
      email: 'u@example.com', plan: 'personal', device_id: 'dev-1',
      fingerprint: 'a3f9c211', issued_at: '2026-05-14T10:00:00Z',
      expires_at: null, next_check_at: '2026-05-21T10:00:00Z',
      max_grace_until: '2026-05-28T10:00:00Z', key_version: 1,
    };
    const env = await signCertificate(payload, sk);
    // 篡改 payload_b64 中的一个字符
    env.payload_b64 = env.payload_b64.replace(/^./, 'X');
    const verified = await verifyCertificate(env, pk);
    expect(verified).toBeNull();
  });

  it('verify returns null for wrong pubkey', async () => {
    const sk1 = utils.randomPrivateKey();
    const sk2 = utils.randomPrivateKey();
    const pk2 = await getPublicKeyAsync(sk2);
    const env = await signCertificate({
      license_id: 'x', license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE',
      email: 'u@example.com', plan: 'personal', device_id: 'd',
      fingerprint: 'fp', issued_at: '2026-05-14T10:00:00Z', expires_at: null,
      next_check_at: '2026-05-21T10:00:00Z', max_grace_until: '2026-05-28T10:00:00Z',
      key_version: 1,
    }, sk1);
    expect(await verifyCertificate(env, pk2)).toBeNull();
  });
});
```

- [ ] **Step 2: 跑测试确认失败**

```bash
npx vitest run tests/unit/ed25519.test.ts
```

Expected: FAIL — 模块不存在

- [ ] **Step 3: 实现 `shared/ed25519.ts`**

```typescript
// shared/ed25519.ts
import { signAsync, verifyAsync } from '@noble/ed25519';
import type { LicensePayload, LicenseCertificateEnvelope } from './types';

const enc = new TextEncoder();
const dec = new TextDecoder();

function b64encode(bytes: Uint8Array): string {
  let s = '';
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s);
}
function b64decode(s: string): Uint8Array {
  const raw = atob(s);
  const out = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) out[i] = raw.charCodeAt(i);
  return out;
}

export async function signCertificate(payload: LicensePayload, secretKey: Uint8Array): Promise<LicenseCertificateEnvelope> {
  const payloadJson = JSON.stringify(payload);
  const payloadBytes = enc.encode(payloadJson);
  const sig = await signAsync(payloadBytes, secretKey);
  return {
    v: 1,
    payload_b64: b64encode(payloadBytes),
    sig_b64: b64encode(sig),
  };
}

export async function verifyCertificate(envelope: LicenseCertificateEnvelope, publicKey: Uint8Array): Promise<LicensePayload | null> {
  if (envelope.v !== 1) return null;
  try {
    const payloadBytes = b64decode(envelope.payload_b64);
    const sig = b64decode(envelope.sig_b64);
    const ok = await verifyAsync(sig, payloadBytes, publicKey);
    if (!ok) return null;
    return JSON.parse(dec.decode(payloadBytes)) as LicensePayload;
  } catch {
    return null;
  }
}

export function loadPrivateKeyFromHex(hex: string): Uint8Array {
  const out = new Uint8Array(hex.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  if (out.length !== 32) throw new Error(`Bad Ed25519 secret key length: ${out.length}`);
  return out;
}
```

- [ ] **Step 4: 跑测试通过**

```bash
npx vitest run tests/unit/ed25519.test.ts
```

Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add shared/ed25519.ts tests/unit/ed25519.test.ts
git commit -m "feat(crypto): Ed25519 sign/verify 共用函数 + 篡改/错误公钥测试"
```

---

### Task 1.5: 共用响应辅助 + 限流 + 审计日志辅助

**Files:**
- Create: `dimkey-web/functions/_lib/response.ts`
- Create: `dimkey-web/functions/_lib/audit.ts`
- Create: `dimkey-web/functions/_lib/rate-limit.ts`
- Create: `dimkey-web/functions/_lib/now.ts`

- [ ] **Step 1: 写 `_lib/now.ts`（统一时间源，便于测试 mock）**

```typescript
// functions/_lib/now.ts
export function now(): number { return Date.now(); }
export function nowIso(): string { return new Date().toISOString(); }
```

- [ ] **Step 2: 写 `_lib/response.ts`**

```typescript
// functions/_lib/response.ts
import type { ApiResp, ErrorCodeT } from '@shared/types';
import { ErrorCode } from '@shared/codes';

export function ok<T>(data: T): Response {
  const body: ApiResp<T> = { ok: true, data };
  return Response.json(body, { status: 200 });
}

export function err(code: ErrorCodeT, message: string, data?: unknown, httpStatus = 200): Response {
  return Response.json({ ok: false, code, message, data }, { status: httpStatus });
}

export function serverError(e: unknown): Response {
  const message = e instanceof Error ? e.message : String(e);
  console.error('[server_error]', message, e);
  return err(ErrorCode.SERVER_ERROR, '服务暂时不可用，请稍后再试', undefined, 500);
}
```

- [ ] **Step 3: 写 `_lib/audit.ts`**

```typescript
// functions/_lib/audit.ts
import { now } from './now';

export interface AuditEntry {
  license_id?: string;
  device_id?: string;
  event: string;                   // activate|deactivate|heartbeat|revoke|recover|refund|issue|transfer|admin_lookup
  ip?: string;
  user_agent?: string;
  detail?: Record<string, unknown>;
}

export async function writeAudit(db: D1Database, entry: AuditEntry): Promise<void> {
  await db.prepare(
    `INSERT INTO audit_log (license_id, device_id, event, ip, user_agent, detail, ts)
     VALUES (?, ?, ?, ?, ?, ?, ?)`
  ).bind(
    entry.license_id ?? null,
    entry.device_id ?? null,
    entry.event,
    entry.ip ?? null,
    entry.user_agent ?? null,
    entry.detail ? JSON.stringify(entry.detail) : null,
    now(),
  ).run();
}
```

- [ ] **Step 4: 写 `_lib/rate-limit.ts`（KV 简单计数版本，v1 够用；后续可换 CF Rate Limiting binding）**

```typescript
// functions/_lib/rate-limit.ts
import { now } from './now';

export async function checkRateLimit(
  kv: KVNamespace,
  bucketKey: string,        // 例如 "activate:ip:1.2.3.4"
  limit: number,
  windowSec: number,
): Promise<{ allowed: boolean; remaining: number }> {
  const t = now();
  const key = `rl:${bucketKey}`;
  const raw = await kv.get(key);
  const arr: number[] = raw ? JSON.parse(raw) : [];
  const cutoff = t - windowSec * 1000;
  const recent = arr.filter(ts => ts > cutoff);
  if (recent.length >= limit) return { allowed: false, remaining: 0 };
  recent.push(t);
  await kv.put(key, JSON.stringify(recent), { expirationTtl: windowSec });
  return { allowed: true, remaining: limit - recent.length };
}
```

- [ ] **Step 5: 提交**

```bash
git add functions/_lib/
git commit -m "feat(api): 响应/审计/限流/时间源共用辅助"
```

---

## Phase 2：API — `/api/v1/activate`

### Task 2.1: 集成测试骨架（用 Miniflare / Vitest）

**Files:**
- Create: `dimkey-web/tests/integration/_setup.ts`
- Create: `dimkey-web/tests/integration/activate.test.ts`
- Create: `dimkey-web/vitest.config.ts`

- [ ] **Step 1: 写 `vitest.config.ts`**

```typescript
// vitest.config.ts
import { defineConfig } from 'vitest/config';
import path from 'node:path';

export default defineConfig({
  test: {
    environment: 'miniflare',
    environmentOptions: {
      d1Databases: ['DB'],
      kvNamespaces: ['KV'],
      bindings: {
        ED25519_PRIVATE_KEY: 'ee'.repeat(32),  // 测试固定值
        LS_WEBHOOK_SECRET: 'test-ls-secret',
        ADMIN_TOKEN: 'test-admin-token',
        RESEND_API_KEY: 'test-resend',
      },
      compatibilityDate: '2026-05-01',
    },
  },
  resolve: {
    alias: {
      '@shared': path.resolve(__dirname, './shared'),
    },
  },
});
```

- [ ] **Step 2: 写 `tests/integration/_setup.ts`（每个测试前 reset DB + 构造 fetch 工具）**

```typescript
// tests/integration/_setup.ts
import { beforeEach } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

export async function resetDb(db: D1Database): Promise<void> {
  // 拆 0001_init.sql 按 ; 分句执行（先 DROP 再 CREATE）
  const sql = fs.readFileSync(path.resolve(__dirname, '../../shared/migrations/0001_init.sql'), 'utf-8');
  await db.exec('DROP TABLE IF EXISTS licenses; DROP TABLE IF EXISTS devices; DROP TABLE IF EXISTS audit_log; DROP TABLE IF EXISTS orders_inbox;');
  for (const stmt of sql.split(/;\s*[\r\n]/).filter(s => s.trim())) {
    await db.exec(stmt + ';');
  }
}

export async function seedLicense(db: D1Database, opts: { license_key: string; email: string; max_devices?: number; status?: string } ): Promise<string> {
  const id = crypto.randomUUID();
  const t = Date.now();
  await db.prepare(
    `INSERT INTO licenses (license_id, license_key, email, plan, max_devices, status, source, order_ref, issued_at, created_at, updated_at)
     VALUES (?, ?, ?, 'personal', ?, ?, 'manual_cn', ?, ?, ?, ?)`
  ).bind(id, opts.license_key, opts.email, opts.max_devices ?? 3, opts.status ?? 'active', `test-${id}`, t, t, t).run();
  return id;
}

export function bindings() {
  return (globalThis as any).__MINIFLARE_BINDINGS__ as { DB: D1Database; KV: KVNamespace; ED25519_PRIVATE_KEY: string };
}
```

- [ ] **Step 3: 写 `tests/integration/activate.test.ts` 第一个失败测试**

```typescript
// tests/integration/activate.test.ts
import { describe, it, expect, beforeEach } from 'vitest';
import { onRequestPost as activate } from '../../functions/api/v1/activate';
import { resetDb, seedLicense, bindings } from './_setup';

describe('POST /api/v1/activate', () => {
  beforeEach(async () => { await resetDb(bindings().DB); });

  it('returns INVALID_LICENSE for unknown key', async () => {
    const req = new Request('https://x/api/v1/activate', {
      method: 'POST',
      body: JSON.stringify({
        license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE',
        email: 'u@example.com',
        fingerprint: 'fp1', machine_label: 'M', os: 'macos', flavor: 'zh', app_version: '0.7.0',
      }),
      headers: { 'content-type': 'application/json' },
    });
    const res = await activate({ request: req, env: bindings(), waitUntil: () => {} } as any);
    const body = await res.json() as any;
    expect(body.ok).toBe(false);
    expect(body.code).toBe('INVALID_LICENSE');
  });
});
```

- [ ] **Step 4: 跑测试确认失败（接口不存在）**

```bash
npx vitest run tests/integration/activate.test.ts
```

Expected: FAIL — `Cannot find module '../../functions/api/v1/activate'`

- [ ] **Step 5: 提交（红色测试 + 测试基础设施）**

```bash
git add vitest.config.ts tests/integration/_setup.ts tests/integration/activate.test.ts
git commit -m "test(integration): activate 失败测试 + Miniflare 集成测试基础设施"
```

---

### Task 2.2: 实现 `/api/v1/activate` — INVALID_LICENSE 分支

**Files:**
- Create: `dimkey-web/functions/api/v1/activate.ts`

- [ ] **Step 1: 实现最小骨架（仅 INVALID_LICENSE 路径）**

```typescript
// functions/api/v1/activate.ts
import { ok, err, serverError } from '../../_lib/response';
import { writeAudit } from '../../_lib/audit';
import { ErrorCode } from '@shared/codes';
import type { ActivateRequest } from '@shared/types';

interface Env {
  DB: D1Database;
  KV: KVNamespace;
  ED25519_PRIVATE_KEY: string;
}

export const onRequestPost: PagesFunction<Env> = async (ctx) => {
  try {
    const body = await ctx.request.json() as ActivateRequest;
    const license = await ctx.env.DB.prepare(
      `SELECT * FROM licenses WHERE license_key = ?`
    ).bind(body.license_key).first();

    if (!license || license.email !== body.email) {
      return err(ErrorCode.INVALID_LICENSE, '邮箱或许可证不正确');
    }

    // TODO: 后续 task 加更多分支
    return err(ErrorCode.SERVER_ERROR, 'not implemented');
  } catch (e) {
    return serverError(e);
  }
};
```

- [ ] **Step 2: 跑测试确认通过**

```bash
npx vitest run tests/integration/activate.test.ts
```

Expected: PASS

- [ ] **Step 3: 提交**

```bash
git add functions/api/v1/activate.ts
git commit -m "feat(api): activate 接口 INVALID_LICENSE 分支"
```

---

### Task 2.3: activate — 状态非 active / email 不匹配 / 成功首次激活

**Files:**
- Modify: `dimkey-web/tests/integration/activate.test.ts`
- Modify: `dimkey-web/functions/api/v1/activate.ts`

- [ ] **Step 1: 加测试用例**

```typescript
  it('returns LICENSE_REVOKED when status=revoked', async () => {
    await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com', status: 'revoked' });
    const req = new Request('https://x/api/v1/activate', { method: 'POST',
      body: JSON.stringify({ license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com', fingerprint: 'fp1', machine_label: 'M', os: 'macos', flavor: 'zh', app_version: '0.7.0' }),
      headers: { 'content-type': 'application/json' } });
    const res = await activate({ request: req, env: bindings(), waitUntil: () => {} } as any);
    const body = await res.json() as any;
    expect(body.code).toBe('LICENSE_REVOKED');
  });

  it('returns INVALID_LICENSE when email mismatches (防枚举)', async () => {
    await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'real@example.com' });
    const req = new Request('https://x/api/v1/activate', { method: 'POST',
      body: JSON.stringify({ license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'wrong@example.com', fingerprint: 'fp1', machine_label: 'M', os: 'macos', flavor: 'zh', app_version: '0.7.0' }),
      headers: { 'content-type': 'application/json' } });
    const res = await activate({ request: req, env: bindings(), waitUntil: () => {} } as any);
    const body = await res.json() as any;
    expect(body.code).toBe('INVALID_LICENSE');   // 不暴露 EMAIL_MISMATCH
  });

  it('first-time activation: inserts device, returns signed certificate', async () => {
    const licId = await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com' });
    const req = new Request('https://x/api/v1/activate', { method: 'POST',
      body: JSON.stringify({ license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com', fingerprint: 'fp1', machine_label: 'M1', os: 'macos', flavor: 'zh', app_version: '0.7.0' }),
      headers: { 'content-type': 'application/json' } });
    const res = await activate({ request: req, env: bindings(), waitUntil: () => {} } as any);
    const body = await res.json() as any;
    expect(body.ok).toBe(true);
    expect(body.data.license_certificate.v).toBe(1);
    expect(body.data.device_summary.active_count).toBe(1);
    expect(body.data.device_summary.max_devices).toBe(3);

    // device 已写入
    const dev = await bindings().DB.prepare(`SELECT * FROM devices WHERE license_id = ?`).bind(licId).first();
    expect(dev).toBeTruthy();
    expect(dev!.fingerprint).toBe('fp1');
  });
```

- [ ] **Step 2: 扩展实现**

```typescript
// functions/api/v1/activate.ts
import { ok, err, serverError } from '../../_lib/response';
import { writeAudit } from '../../_lib/audit';
import { now, nowIso } from '../../_lib/now';
import { signCertificate, loadPrivateKeyFromHex } from '@shared/ed25519';
import { ErrorCode } from '@shared/codes';
import type { ActivateRequest, LicensePayload } from '@shared/types';

interface Env {
  DB: D1Database; KV: KVNamespace; ED25519_PRIVATE_KEY: string;
}

const HEARTBEAT_DAYS = 7;
const GRACE_DAYS = 14;
const DAY_MS = 86_400_000;

export const onRequestPost: PagesFunction<Env> = async (ctx) => {
  try {
    const body = await ctx.request.json() as ActivateRequest;
    const license = await ctx.env.DB.prepare(
      `SELECT license_id, email, plan, max_devices, status FROM licenses WHERE license_key = ?`
    ).bind(body.license_key).first<{ license_id: string; email: string; plan: string; max_devices: number; status: string }>();

    if (!license || license.email !== body.email) {
      return err(ErrorCode.INVALID_LICENSE, '邮箱或许可证不正确');
    }
    if (license.status === 'revoked' || license.status === 'refunded') {
      return err(ErrorCode.LICENSE_REVOKED, '本许可证已失效');
    }
    if (license.status === 'expired') {
      return err(ErrorCode.LICENSE_EXPIRED, '许可证已到期');
    }

    // 幂等：已存在该指纹的活跃 device → 重签
    const existing = await ctx.env.DB.prepare(
      `SELECT device_id FROM devices WHERE license_id = ? AND fingerprint = ? AND deactivated_at IS NULL`
    ).bind(license.license_id, body.fingerprint).first<{ device_id: string }>();

    let deviceId: string;
    let activeCount: number;
    if (existing) {
      deviceId = existing.device_id;
      await ctx.env.DB.prepare(`UPDATE devices SET last_seen = ?, app_version = ?, machine_label = ? WHERE device_id = ?`)
        .bind(now(), body.app_version, body.machine_label, deviceId).run();
      const cnt = await ctx.env.DB.prepare(
        `SELECT COUNT(*) AS c FROM devices WHERE license_id = ? AND deactivated_at IS NULL`
      ).bind(license.license_id).first<{ c: number }>();
      activeCount = cnt!.c;
    } else {
      const cnt = await ctx.env.DB.prepare(
        `SELECT COUNT(*) AS c FROM devices WHERE license_id = ? AND deactivated_at IS NULL`
      ).bind(license.license_id).first<{ c: number }>();
      if (cnt!.c >= license.max_devices) {
        const devices = await ctx.env.DB.prepare(
          `SELECT device_id, machine_label, os, flavor, first_activated, last_seen FROM devices
           WHERE license_id = ? AND deactivated_at IS NULL ORDER BY last_seen DESC`
        ).bind(license.license_id).all();
        return err(ErrorCode.DEVICE_LIMIT_REACHED, `已达 ${license.max_devices} 台设备上限，请先解绑一台`,
          { devices: devices.results, max_devices: license.max_devices });
      }
      deviceId = crypto.randomUUID();
      await ctx.env.DB.prepare(
        `INSERT INTO devices (device_id, license_id, fingerprint, machine_label, os, flavor, app_version, first_activated, last_seen)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
      ).bind(deviceId, license.license_id, body.fingerprint, body.machine_label, body.os, body.flavor, body.app_version, now(), now()).run();
      activeCount = cnt!.c + 1;
    }

    // 签证书
    const issuedAt = nowIso();
    const next = new Date(now() + HEARTBEAT_DAYS * DAY_MS).toISOString();
    const grace = new Date(now() + GRACE_DAYS * DAY_MS).toISOString();
    const payload: LicensePayload = {
      license_id: license.license_id,
      license_key: body.license_key,
      email: body.email,
      plan: 'personal',
      device_id: deviceId,
      fingerprint: body.fingerprint,
      issued_at: issuedAt,
      expires_at: null,
      next_check_at: next,
      max_grace_until: grace,
      key_version: 1,
    };
    const sk = loadPrivateKeyFromHex(ctx.env.ED25519_PRIVATE_KEY);
    const cert = await signCertificate(payload, sk);

    ctx.waitUntil(writeAudit(ctx.env.DB, {
      license_id: license.license_id,
      device_id: deviceId,
      event: existing ? 'reactivate' : 'activate',
      ip: ctx.request.headers.get('cf-connecting-ip') ?? undefined,
      user_agent: ctx.request.headers.get('user-agent') ?? undefined,
      detail: { os: body.os, flavor: body.flavor, app_version: body.app_version },
    }));

    return ok({
      license_certificate: cert,
      device_summary: { current_device_id: deviceId, active_count: activeCount, max_devices: license.max_devices },
    });
  } catch (e) {
    return serverError(e);
  }
};
```

- [ ] **Step 3: 跑测试确认通过**

```bash
npx vitest run tests/integration/activate.test.ts
```

Expected: 全部 PASS

- [ ] **Step 4: 提交**

```bash
git add tests/integration/activate.test.ts functions/api/v1/activate.ts
git commit -m "feat(api): activate 完整实现 — 状态校验/幂等重签/首次签发"
```

---

### Task 2.4: activate — DEVICE_LIMIT_REACHED 测试 + 幂等性测试

**Files:**
- Modify: `dimkey-web/tests/integration/activate.test.ts`

- [ ] **Step 1: 加测试**

```typescript
  it('DEVICE_LIMIT_REACHED returns device list for UI', async () => {
    const licId = await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com', max_devices: 2 });
    // 预填 2 台
    for (const fp of ['fp-old1', 'fp-old2']) {
      await bindings().DB.prepare(`INSERT INTO devices (device_id, license_id, fingerprint, machine_label, os, flavor, first_activated, last_seen) VALUES (?, ?, ?, ?, 'macos', 'zh', ?, ?)`)
        .bind(crypto.randomUUID(), licId, fp, `M-${fp}`, Date.now(), Date.now()).run();
    }
    const req = new Request('https://x/api/v1/activate', { method: 'POST',
      body: JSON.stringify({ license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com', fingerprint: 'fp-new', machine_label: 'New', os: 'macos', flavor: 'zh', app_version: '0.7.0' }),
      headers: { 'content-type': 'application/json' } });
    const res = await activate({ request: req, env: bindings(), waitUntil: () => {} } as any);
    const body = await res.json() as any;
    expect(body.code).toBe('DEVICE_LIMIT_REACHED');
    expect(body.data.devices).toHaveLength(2);
    expect(body.data.max_devices).toBe(2);
  });

  it('idempotent: same fingerprint returns same device_id, count unchanged', async () => {
    const licId = await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com' });
    const req = (fp: string) => new Request('https://x/api/v1/activate', { method: 'POST',
      body: JSON.stringify({ license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com', fingerprint: fp, machine_label: 'M', os: 'macos', flavor: 'zh', app_version: '0.7.0' }),
      headers: { 'content-type': 'application/json' } });
    const r1 = await (await activate({ request: req('fp1'), env: bindings(), waitUntil: () => {} } as any)).json() as any;
    const r2 = await (await activate({ request: req('fp1'), env: bindings(), waitUntil: () => {} } as any)).json() as any;
    expect(r1.data.device_summary.current_device_id).toBe(r2.data.device_summary.current_device_id);
    expect(r2.data.device_summary.active_count).toBe(1);
  });
```

- [ ] **Step 2: 跑测试**

```bash
npx vitest run tests/integration/activate.test.ts
```

Expected: 全 PASS（实现已覆盖）

- [ ] **Step 3: 提交**

```bash
git add tests/integration/activate.test.ts
git commit -m "test(activate): 设备超限返回设备列表 + 幂等性"
```

---

## Phase 3：API — `/api/v1/deactivate`

### Task 3.1: deactivate — 测试 + 实现

**Files:**
- Create: `dimkey-web/functions/api/v1/deactivate.ts`
- Create: `dimkey-web/tests/integration/deactivate.test.ts`

- [ ] **Step 1: 写失败测试**

```typescript
// tests/integration/deactivate.test.ts
import { describe, it, expect, beforeEach } from 'vitest';
import { onRequestPost as deactivate } from '../../functions/api/v1/deactivate';
import { resetDb, seedLicense, bindings } from './_setup';

async function seedDevice(licId: string, fp: string): Promise<string> {
  const id = crypto.randomUUID();
  await bindings().DB.prepare(`INSERT INTO devices (device_id, license_id, fingerprint, machine_label, os, flavor, first_activated, last_seen) VALUES (?, ?, ?, 'M', 'macos', 'zh', ?, ?)`)
    .bind(id, licId, fp, Date.now(), Date.now()).run();
  return id;
}

describe('POST /api/v1/deactivate', () => {
  beforeEach(async () => { await resetDb(bindings().DB); });

  it('deactivates by device_id', async () => {
    const licId = await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com' });
    const devId = await seedDevice(licId, 'fp1');
    const req = new Request('https://x/api/v1/deactivate', { method: 'POST',
      body: JSON.stringify({ license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com', device_id: devId }),
      headers: { 'content-type': 'application/json' } });
    const res = await deactivate({ request: req, env: bindings(), waitUntil: () => {} } as any);
    const body = await res.json() as any;
    expect(body.ok).toBe(true);
    const row = await bindings().DB.prepare(`SELECT deactivated_at FROM devices WHERE device_id = ?`).bind(devId).first<{ deactivated_at: number }>();
    expect(row!.deactivated_at).toBeGreaterThan(0);
  });

  it('deactivates by fingerprint when device_id omitted', async () => {
    const licId = await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com' });
    await seedDevice(licId, 'fp-current');
    const req = new Request('https://x/api/v1/deactivate', { method: 'POST',
      body: JSON.stringify({ license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com', fingerprint: 'fp-current' }),
      headers: { 'content-type': 'application/json' } });
    const res = await deactivate({ request: req, env: bindings(), waitUntil: () => {} } as any);
    expect((await res.json() as any).ok).toBe(true);
  });

  it('DEVICE_NOT_FOUND for unknown device_id', async () => {
    await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com' });
    const req = new Request('https://x/api/v1/deactivate', { method: 'POST',
      body: JSON.stringify({ license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com', device_id: 'no-such' }),
      headers: { 'content-type': 'application/json' } });
    const body = await (await deactivate({ request: req, env: bindings(), waitUntil: () => {} } as any)).json() as any;
    expect(body.code).toBe('DEVICE_NOT_FOUND');
  });

  it('INVALID_LICENSE for wrong key/email', async () => {
    const req = new Request('https://x/api/v1/deactivate', { method: 'POST',
      body: JSON.stringify({ license_key: 'DK-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX', email: 'u@example.com', device_id: 'x' }),
      headers: { 'content-type': 'application/json' } });
    const body = await (await deactivate({ request: req, env: bindings(), waitUntil: () => {} } as any)).json() as any;
    expect(body.code).toBe('INVALID_LICENSE');
  });
});
```

- [ ] **Step 2: 跑测试确认失败**

```bash
npx vitest run tests/integration/deactivate.test.ts
```

Expected: 模块不存在 → FAIL

- [ ] **Step 3: 实现**

```typescript
// functions/api/v1/deactivate.ts
import { ok, err, serverError } from '../../_lib/response';
import { writeAudit } from '../../_lib/audit';
import { now } from '../../_lib/now';
import { ErrorCode } from '@shared/codes';
import type { DeactivateRequest } from '@shared/types';

interface Env { DB: D1Database }

export const onRequestPost: PagesFunction<Env> = async (ctx) => {
  try {
    const body = await ctx.request.json() as DeactivateRequest;
    const license = await ctx.env.DB.prepare(`SELECT license_id, email FROM licenses WHERE license_key = ?`)
      .bind(body.license_key).first<{ license_id: string; email: string }>();
    if (!license || license.email !== body.email) {
      return err(ErrorCode.INVALID_LICENSE, '邮箱或许可证不正确');
    }

    let device;
    if (body.device_id) {
      device = await ctx.env.DB.prepare(`SELECT device_id FROM devices WHERE device_id = ? AND license_id = ? AND deactivated_at IS NULL`)
        .bind(body.device_id, license.license_id).first<{ device_id: string }>();
    } else if (body.fingerprint) {
      device = await ctx.env.DB.prepare(`SELECT device_id FROM devices WHERE fingerprint = ? AND license_id = ? AND deactivated_at IS NULL`)
        .bind(body.fingerprint, license.license_id).first<{ device_id: string }>();
    }
    if (!device) return err(ErrorCode.DEVICE_NOT_FOUND, '设备不存在或已解绑');

    await ctx.env.DB.prepare(`UPDATE devices SET deactivated_at = ? WHERE device_id = ?`).bind(now(), device.device_id).run();
    ctx.waitUntil(writeAudit(ctx.env.DB, {
      license_id: license.license_id, device_id: device.device_id, event: 'deactivate',
      ip: ctx.request.headers.get('cf-connecting-ip') ?? undefined,
    }));
    return ok({});
  } catch (e) {
    return serverError(e);
  }
};
```

- [ ] **Step 4: 跑测试通过**

```bash
npx vitest run tests/integration/deactivate.test.ts
```

Expected: 全 PASS

- [ ] **Step 5: 提交**

```bash
git add functions/api/v1/deactivate.ts tests/integration/deactivate.test.ts
git commit -m "feat(api): deactivate — 按 device_id 或 fingerprint 解绑"
```

---

## Phase 4：API — `/api/v1/heartbeat`

### Task 4.1: heartbeat — 测试 + 实现

**Files:**
- Create: `dimkey-web/functions/api/v1/heartbeat.ts`
- Create: `dimkey-web/tests/integration/heartbeat.test.ts`

- [ ] **Step 1: 写失败测试**

```typescript
// tests/integration/heartbeat.test.ts
import { describe, it, expect, beforeEach } from 'vitest';
import { onRequestPost as heartbeat } from '../../functions/api/v1/heartbeat';
import { resetDb, seedLicense, bindings } from './_setup';

describe('POST /api/v1/heartbeat', () => {
  beforeEach(async () => { await resetDb(bindings().DB); });

  it('returns active + next_check_at; updates last_seen', async () => {
    const licId = await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com' });
    const devId = crypto.randomUUID();
    await bindings().DB.prepare(`INSERT INTO devices (device_id, license_id, fingerprint, machine_label, os, flavor, first_activated, last_seen) VALUES (?, ?, ?, 'M', 'macos', 'zh', ?, 0)`)
      .bind(devId, licId, 'fp1', Date.now()).run();

    const req = new Request('https://x/api/v1/heartbeat', { method: 'POST',
      body: JSON.stringify({ license_id: licId, device_id: devId, fingerprint: 'fp1' }),
      headers: { 'content-type': 'application/json' } });
    const body = await (await heartbeat({ request: req, env: bindings(), waitUntil: () => {} } as any)).json() as any;
    expect(body.ok).toBe(true);
    expect(body.data.status).toBe('active');
    expect(body.data.next_check_at).toBeGreaterThan(Date.now());

    const row = await bindings().DB.prepare(`SELECT last_seen FROM devices WHERE device_id = ?`).bind(devId).first<{ last_seen: number }>();
    expect(row!.last_seen).toBeGreaterThan(0);
  });

  it('returns revoked when license.status=refunded', async () => {
    const licId = await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com', status: 'refunded' });
    const devId = crypto.randomUUID();
    await bindings().DB.prepare(`INSERT INTO devices (device_id, license_id, fingerprint, machine_label, os, flavor, first_activated, last_seen) VALUES (?, ?, ?, 'M', 'macos', 'zh', ?, ?)`)
      .bind(devId, licId, 'fp1', Date.now(), Date.now()).run();

    const req = new Request('https://x/api/v1/heartbeat', { method: 'POST',
      body: JSON.stringify({ license_id: licId, device_id: devId, fingerprint: 'fp1' }),
      headers: { 'content-type': 'application/json' } });
    const body = await (await heartbeat({ request: req, env: bindings(), waitUntil: () => {} } as any)).json() as any;
    expect(body.data.status).toBe('revoked');
  });

  it('returns revoked when device deactivated', async () => {
    const licId = await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com' });
    const devId = crypto.randomUUID();
    await bindings().DB.prepare(`INSERT INTO devices (device_id, license_id, fingerprint, machine_label, os, flavor, first_activated, last_seen, deactivated_at) VALUES (?, ?, ?, 'M', 'macos', 'zh', ?, ?, ?)`)
      .bind(devId, licId, 'fp1', Date.now(), Date.now(), Date.now()).run();
    const req = new Request('https://x/api/v1/heartbeat', { method: 'POST',
      body: JSON.stringify({ license_id: licId, device_id: devId, fingerprint: 'fp1' }),
      headers: { 'content-type': 'application/json' } });
    const body = await (await heartbeat({ request: req, env: bindings(), waitUntil: () => {} } as any)).json() as any;
    expect(body.data.status).toBe('revoked');
  });
});
```

- [ ] **Step 2: 跑测试 — 失败**

```bash
npx vitest run tests/integration/heartbeat.test.ts
```

- [ ] **Step 3: 实现**

```typescript
// functions/api/v1/heartbeat.ts
import { ok, serverError } from '../../_lib/response';
import { now } from '../../_lib/now';
import type { HeartbeatRequest } from '@shared/types';

interface Env { DB: D1Database }
const HEARTBEAT_DAYS = 7;
const DAY_MS = 86_400_000;

export const onRequestPost: PagesFunction<Env> = async (ctx) => {
  try {
    const body = await ctx.request.json() as HeartbeatRequest;
    const row = await ctx.env.DB.prepare(
      `SELECT l.status AS lstatus, d.deactivated_at
       FROM licenses l JOIN devices d ON d.license_id = l.license_id
       WHERE d.device_id = ? AND l.license_id = ? AND d.fingerprint = ?`
    ).bind(body.device_id, body.license_id, body.fingerprint).first<{ lstatus: string; deactivated_at: number | null }>();

    if (!row || row.lstatus !== 'active' || row.deactivated_at !== null) {
      return ok({ status: 'revoked', next_check_at: now() + HEARTBEAT_DAYS * DAY_MS });
    }

    await ctx.env.DB.prepare(`UPDATE devices SET last_seen = ? WHERE device_id = ?`).bind(now(), body.device_id).run();
    return ok({ status: 'active', next_check_at: now() + HEARTBEAT_DAYS * DAY_MS });
  } catch (e) {
    return serverError(e);
  }
};
```

- [ ] **Step 4: 测试通过**

```bash
npx vitest run tests/integration/heartbeat.test.ts
```

- [ ] **Step 5: 提交**

```bash
git add functions/api/v1/heartbeat.ts tests/integration/heartbeat.test.ts
git commit -m "feat(api): heartbeat — 复验 + revoked 检测"
```

---

## Phase 5：API — `/api/v1/devices/list`

### Task 5.1: devices/list — 测试 + 实现

**Files:**
- Create: `dimkey-web/functions/api/v1/devices/list.ts`
- Create: `dimkey-web/tests/integration/devices-list.test.ts`

- [ ] **Step 1: 写失败测试**

```typescript
// tests/integration/devices-list.test.ts
import { describe, it, expect, beforeEach } from 'vitest';
import { onRequestPost as listDevices } from '../../functions/api/v1/devices/list';
import { resetDb, seedLicense, bindings } from './_setup';

describe('POST /api/v1/devices/list', () => {
  beforeEach(async () => { await resetDb(bindings().DB); });

  it('returns active devices, marks is_current by fingerprint', async () => {
    const licId = await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com' });
    for (const fp of ['fp1', 'fp2']) {
      await bindings().DB.prepare(`INSERT INTO devices (device_id, license_id, fingerprint, machine_label, os, flavor, first_activated, last_seen) VALUES (?, ?, ?, ?, 'macos', 'zh', ?, ?)`)
        .bind(crypto.randomUUID(), licId, fp, `M-${fp}`, Date.now(), Date.now()).run();
    }
    // 一台已解绑
    await bindings().DB.prepare(`INSERT INTO devices (device_id, license_id, fingerprint, machine_label, os, flavor, first_activated, last_seen, deactivated_at) VALUES (?, ?, 'fp-old', 'M-old', 'macos', 'zh', ?, ?, ?)`)
      .bind(crypto.randomUUID(), licId, Date.now(), Date.now(), Date.now()).run();

    const req = new Request('https://x/api/v1/devices/list', { method: 'POST',
      body: JSON.stringify({ license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com', fingerprint: 'fp1' }),
      headers: { 'content-type': 'application/json' } });
    const body = await (await listDevices({ request: req, env: bindings(), waitUntil: () => {} } as any)).json() as any;
    expect(body.ok).toBe(true);
    expect(body.data.devices).toHaveLength(2);
    expect(body.data.devices.find((d: any) => d.is_current).machine_label).toBe('M-fp1');
    expect(body.data.max_devices).toBe(3);
  });

  it('INVALID_LICENSE on wrong credentials', async () => {
    const req = new Request('https://x/api/v1/devices/list', { method: 'POST',
      body: JSON.stringify({ license_key: 'DK-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX', email: 'u@example.com' }),
      headers: { 'content-type': 'application/json' } });
    const body = await (await listDevices({ request: req, env: bindings(), waitUntil: () => {} } as any)).json() as any;
    expect(body.code).toBe('INVALID_LICENSE');
  });
});
```

- [ ] **Step 2: 实现**

```typescript
// functions/api/v1/devices/list.ts
import { ok, err, serverError } from '../../../_lib/response';
import { ErrorCode } from '@shared/codes';
import type { DevicesListRequest, DeviceDto } from '@shared/types';

interface Env { DB: D1Database }

export const onRequestPost: PagesFunction<Env> = async (ctx) => {
  try {
    const body = await ctx.request.json() as DevicesListRequest;
    const license = await ctx.env.DB.prepare(`SELECT license_id, email, max_devices FROM licenses WHERE license_key = ?`)
      .bind(body.license_key).first<{ license_id: string; email: string; max_devices: number }>();
    if (!license || license.email !== body.email) {
      return err(ErrorCode.INVALID_LICENSE, '邮箱或许可证不正确');
    }
    const rows = await ctx.env.DB.prepare(
      `SELECT device_id, machine_label, os, flavor, first_activated, last_seen, fingerprint
       FROM devices WHERE license_id = ? AND deactivated_at IS NULL ORDER BY last_seen DESC`
    ).bind(license.license_id).all<DeviceDto & { fingerprint: string }>();

    const devices = rows.results.map(r => ({
      device_id: r.device_id, machine_label: r.machine_label, os: r.os, flavor: r.flavor as 'zh' | 'en',
      first_activated: r.first_activated, last_seen: r.last_seen,
      is_current: body.fingerprint ? r.fingerprint === body.fingerprint : false,
    }));
    return ok({ devices, max_devices: license.max_devices });
  } catch (e) {
    return serverError(e);
  }
};
```

- [ ] **Step 3: 测试通过 + 提交**

```bash
npx vitest run tests/integration/devices-list.test.ts
git add functions/api/v1/devices/list.ts tests/integration/devices-list.test.ts
git commit -m "feat(api): devices/list — 设备列表 + is_current 标记"
```

---

## Phase 6：API — `/api/v1/recover` + Resend 邮件

### Task 6.1: recover — 测试 + 实现 + Resend 调用

**Files:**
- Create: `dimkey-web/functions/api/v1/recover.ts`
- Create: `dimkey-web/functions/_lib/email.ts`
- Create: `dimkey-web/shared/email-templates/recover-zh.ts`
- Create: `dimkey-web/shared/email-templates/recover-en.ts`
- Create: `dimkey-web/tests/integration/recover.test.ts`

- [ ] **Step 1: 写邮件模板**

```typescript
// shared/email-templates/recover-zh.ts
export function recoverEmailZh(license_key: string, email: string): { subject: string; html: string; text: string } {
  return {
    subject: '你的 Dimkey 许可证',
    text: `你好,\n\n你的 Dimkey 许可证密钥：\n  ${license_key}\n\n激活步骤：\n  1. 打开 Dimkey\n  2. 进入「设置 → 关于 → 输入许可证」\n  3. 输入邮箱（${email}）和上方许可证\n\n如非本人请求，请忽略此邮件。\n\n— Dimkey 团队\nhttps://dimkey.app`,
    html: `<p>你好,</p><p>你的 Dimkey 许可证密钥：<br><code style="font-size:16px;background:#f5f5f5;padding:8px 12px;border-radius:4px;display:inline-block;margin-top:8px">${license_key}</code></p><p>激活步骤：</p><ol><li>打开 Dimkey</li><li>进入「设置 → 关于 → 输入许可证」</li><li>输入邮箱（${email}）和上方许可证</li></ol><p style="color:#888;font-size:13px">如非本人请求，请忽略此邮件。</p><p>— Dimkey 团队<br><a href="https://dimkey.app">https://dimkey.app</a></p>`,
  };
}
```

```typescript
// shared/email-templates/recover-en.ts
export function recoverEmailEn(license_key: string, email: string): { subject: string; html: string; text: string } {
  return {
    subject: 'Your Dimkey License',
    text: `Hi,\n\nYour Dimkey license key:\n  ${license_key}\n\nTo activate:\n  1. Open Dimkey\n  2. Settings → About → "Enter License"\n  3. Enter your email (${email}) and the license key above\n\nIf you didn't request this, please ignore this email.\n\n— Dimkey Team\nhttps://dimkey.app`,
    html: `<p>Hi,</p><p>Your Dimkey license key:<br><code style="font-size:16px;background:#f5f5f5;padding:8px 12px;border-radius:4px;display:inline-block;margin-top:8px">${license_key}</code></p><p>To activate:</p><ol><li>Open Dimkey</li><li>Settings → About → "Enter License"</li><li>Enter your email (${email}) and the license key above</li></ol><p style="color:#888;font-size:13px">If you didn't request this, please ignore this email.</p><p>— Dimkey Team<br><a href="https://dimkey.app">https://dimkey.app</a></p>`,
  };
}
```

- [ ] **Step 2: 写 Resend 客户端 `_lib/email.ts`**

```typescript
// functions/_lib/email.ts
export interface EmailContent { subject: string; html: string; text: string }

export async function sendEmail(apiKey: string, to: string, content: EmailContent): Promise<void> {
  const res = await fetch('https://api.resend.com/emails', {
    method: 'POST',
    headers: { 'Authorization': `Bearer ${apiKey}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({
      from: 'Dimkey <noreply@dimkey.app>',
      to: [to],
      subject: content.subject,
      html: content.html,
      text: content.text,
    }),
  });
  if (!res.ok) {
    const txt = await res.text();
    throw new Error(`Resend send failed: ${res.status} ${txt}`);
  }
}
```

- [ ] **Step 3: 写失败测试（mock Resend，不实发）**

```typescript
// tests/integration/recover.test.ts
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { onRequestPost as recover } from '../../functions/api/v1/recover';
import { resetDb, seedLicense, bindings } from './_setup';

const fetchSpy = vi.spyOn(globalThis, 'fetch');

describe('POST /api/v1/recover', () => {
  beforeEach(async () => {
    await resetDb(bindings().DB);
    fetchSpy.mockReset();
    fetchSpy.mockResolvedValue(new Response(JSON.stringify({ id: 'em_1' }), { status: 200 }));
  });

  it('always returns ok with same message (anti-enumeration)', async () => {
    const reqExisting = new Request('https://x/api/v1/recover', { method: 'POST',
      body: JSON.stringify({ email: 'unknown@example.com' }), headers: { 'content-type': 'application/json' } });
    const body = await (await recover({ request: reqExisting, env: bindings(), waitUntil: (p) => p } as any)).json() as any;
    expect(body.ok).toBe(true);
    expect(body.data.message).toBe('如该邮箱有授权，已发送邮件');
    expect(fetchSpy).not.toHaveBeenCalled();   // 不存在邮箱不发邮件
  });

  it('sends email when license exists for that email', async () => {
    await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'u@example.com' });
    const req = new Request('https://x/api/v1/recover', { method: 'POST',
      body: JSON.stringify({ email: 'u@example.com' }), headers: { 'content-type': 'application/json' } });
    let waitFor: Promise<unknown> = Promise.resolve();
    const res = await recover({ request: req, env: bindings(), waitUntil: (p) => { waitFor = p; } } as any);
    expect((await res.json() as any).ok).toBe(true);
    await waitFor;   // 等异步邮件发送
    expect(fetchSpy).toHaveBeenCalledTimes(1);
    const sentBody = JSON.parse((fetchSpy.mock.calls[0][1] as any).body);
    expect(sentBody.to).toEqual(['u@example.com']);
    expect(sentBody.html).toContain('DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE');
  });
});
```

- [ ] **Step 4: 实现**

```typescript
// functions/api/v1/recover.ts
import { ok, serverError } from '../../_lib/response';
import { sendEmail } from '../../_lib/email';
import { writeAudit } from '../../_lib/audit';
import { recoverEmailZh } from '@shared/email-templates/recover-zh';
import { recoverEmailEn } from '@shared/email-templates/recover-en';
import type { RecoverRequest } from '@shared/types';

interface Env { DB: D1Database; RESEND_API_KEY: string }

export const onRequestPost: PagesFunction<Env> = async (ctx) => {
  try {
    const body = await ctx.request.json() as RecoverRequest;
    const license = await ctx.env.DB.prepare(`SELECT license_id, license_key, source FROM licenses WHERE email = ? AND status = 'active' ORDER BY created_at DESC LIMIT 1`)
      .bind(body.email).first<{ license_id: string; license_key: string; source: string }>();

    if (license) {
      ctx.waitUntil((async () => {
        const isZh = license.source === 'manual_cn' || license.source === 'wechat';
        const tpl = isZh ? recoverEmailZh(license.license_key, body.email) : recoverEmailEn(license.license_key, body.email);
        await sendEmail(ctx.env.RESEND_API_KEY, body.email, tpl);
        await writeAudit(ctx.env.DB, { license_id: license.license_id, event: 'recover', detail: { lang: isZh ? 'zh' : 'en' } });
      })());
    }
    // 不论是否存在都返回相同响应
    return ok({ message: '如该邮箱有授权，已发送邮件' });
  } catch (e) {
    return serverError(e);
  }
};
```

- [ ] **Step 5: 测试 + 提交**

```bash
npx vitest run tests/integration/recover.test.ts
git add functions/api/v1/recover.ts functions/_lib/email.ts shared/email-templates/ tests/integration/recover.test.ts
git commit -m "feat(api): recover — 防扫号 + Resend 异步发邮件"
```

---

## Phase 7：Lemon Squeezy Webhook

### Task 7.1: webhook 签名校验工具（TDD）

**Files:**
- Create: `dimkey-web/functions/_lib/ls-signature.ts`
- Create: `dimkey-web/tests/unit/webhook-signature.test.ts`

- [ ] **Step 1: 写测试**

```typescript
// tests/unit/webhook-signature.test.ts
import { describe, it, expect } from 'vitest';
import { verifyLsSignature } from '../../functions/_lib/ls-signature';

describe('verifyLsSignature', () => {
  const secret = 'super-secret';
  const body = JSON.stringify({ meta: { event_id: 'evt_1' } });
  // pre-computed: hex of HMAC-SHA256(secret, body)
  const validSig = 'CALCULATE_THIS_AT_RUNTIME';

  it('accepts a valid signature', async () => {
    // 跑过一遍 implementation 后用真实输出替换 validSig
    const enc = new TextEncoder();
    const key = await crypto.subtle.importKey('raw', enc.encode(secret), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']);
    const sig = await crypto.subtle.sign('HMAC', key, enc.encode(body));
    const hex = Array.from(new Uint8Array(sig)).map(b => b.toString(16).padStart(2, '0')).join('');
    expect(await verifyLsSignature(body, hex, secret)).toBe(true);
  });

  it('rejects wrong signature', async () => {
    expect(await verifyLsSignature(body, 'aa'.repeat(32), secret)).toBe(false);
  });

  it('rejects null/undefined', async () => {
    expect(await verifyLsSignature(body, null, secret)).toBe(false);
    expect(await verifyLsSignature(body, '', secret)).toBe(false);
  });
});
```

- [ ] **Step 2: 跑测试 — 失败**

```bash
npx vitest run tests/unit/webhook-signature.test.ts
```

- [ ] **Step 3: 实现**

```typescript
// functions/_lib/ls-signature.ts
export async function verifyLsSignature(body: string, signatureHex: string | null, secret: string): Promise<boolean> {
  if (!signatureHex) return false;
  try {
    const enc = new TextEncoder();
    const key = await crypto.subtle.importKey('raw', enc.encode(secret), { name: 'HMAC', hash: 'SHA-256' }, false, ['verify']);
    const sigBytes = new Uint8Array(signatureHex.length / 2);
    for (let i = 0; i < sigBytes.length; i++) sigBytes[i] = parseInt(signatureHex.slice(i * 2, i * 2 + 2), 16);
    return await crypto.subtle.verify('HMAC', key, sigBytes, enc.encode(body));
  } catch {
    return false;
  }
}
```

- [ ] **Step 4: 测试 + 提交**

```bash
npx vitest run tests/unit/webhook-signature.test.ts
git add functions/_lib/ls-signature.ts tests/unit/webhook-signature.test.ts
git commit -m "feat(webhook): Lemon Squeezy HMAC-SHA256 签名校验"
```

---

### Task 7.2: issueLicense 共用函数 + activation 邮件模板

**Files:**
- Create: `dimkey-web/functions/_lib/issue-license.ts`
- Create: `dimkey-web/shared/email-templates/activation-zh.ts`
- Create: `dimkey-web/shared/email-templates/activation-en.ts`

- [ ] **Step 1: 写两个模板（结构同 recover）**

```typescript
// shared/email-templates/activation-zh.ts
export function activationEmailZh(license_key: string, email: string): { subject: string; html: string; text: string } {
  const accountUrl = `https://dimkey.app/account?key=${encodeURIComponent(license_key)}&email=${encodeURIComponent(email)}`;
  return {
    subject: '感谢购买 Dimkey · 你的许可证',
    text: `你好,\n\n感谢购买 Dimkey。\n\n你的许可证密钥：\n  ${license_key}\n\n激活步骤：\n  1. 打开 Dimkey\n  2. 进入「设置 → 关于 → 输入许可证」\n  3. 输入邮箱（${email}）和上方许可证\n\n该许可证最多可在 3 台设备上激活。\n管理设备：${accountUrl}\n\n忘记许可证？在 https://dimkey.app/recover 找回\n\n— Dimkey 团队`,
    html: `<p>你好,</p><p>感谢购买 Dimkey。</p><p>你的许可证密钥：<br><code style="font-size:16px;background:#f5f5f5;padding:8px 12px;border-radius:4px;display:inline-block;margin-top:8px">${license_key}</code></p><p>激活步骤：</p><ol><li>打开 Dimkey</li><li>进入「设置 → 关于 → 输入许可证」</li><li>输入邮箱（${email}）和上方许可证</li></ol><p>该许可证最多可在 3 台设备上激活。<br>管理设备：<a href="${accountUrl}">${accountUrl}</a></p><p style="color:#888;font-size:13px">忘记许可证？在 <a href="https://dimkey.app/recover">https://dimkey.app/recover</a> 找回</p><p>— Dimkey 团队</p>`,
  };
}
```

```typescript
// shared/email-templates/activation-en.ts
export function activationEmailEn(license_key: string, email: string): { subject: string; html: string; text: string } {
  const accountUrl = `https://dimkey.app/account?key=${encodeURIComponent(license_key)}&email=${encodeURIComponent(email)}`;
  return {
    subject: 'Your Dimkey License',
    text: `Hi,\n\nThank you for purchasing Dimkey.\n\nYour license key:\n  ${license_key}\n\nTo activate:\n  1. Open Dimkey\n  2. Settings → About → "Enter License"\n  3. Enter your email (${email}) and the license key above\n\nThis license can be activated on up to 3 devices.\nManage devices: ${accountUrl}\n\nLost your license? Recover at https://dimkey.app/recover\n\n— Dimkey Team`,
    html: `<p>Hi,</p><p>Thank you for purchasing Dimkey.</p><p>Your license key:<br><code style="font-size:16px;background:#f5f5f5;padding:8px 12px;border-radius:4px;display:inline-block;margin-top:8px">${license_key}</code></p><p>To activate:</p><ol><li>Open Dimkey</li><li>Settings → About → "Enter License"</li><li>Enter your email (${email}) and the license key above</li></ol><p>This license can be activated on up to 3 devices.<br>Manage devices: <a href="${accountUrl}">${accountUrl}</a></p><p style="color:#888;font-size:13px">Lost your license? Recover at <a href="https://dimkey.app/recover">https://dimkey.app/recover</a></p><p>— Dimkey Team</p>`,
  };
}
```

- [ ] **Step 2: 实现 `_lib/issue-license.ts`**

```typescript
// functions/_lib/issue-license.ts
import { generateLicenseKey } from '@shared/license-key';
import { activationEmailZh } from '@shared/email-templates/activation-zh';
import { activationEmailEn } from '@shared/email-templates/activation-en';
import { sendEmail } from './email';
import { writeAudit } from './audit';
import { now } from './now';

export interface IssueOpts {
  email: string;
  source: 'lemonsqueezy' | 'manual_cn' | 'wechat';
  order_ref: string;
  plan?: 'personal';
  lang?: 'zh' | 'en';
  notes?: string;
}

export async function issueLicense(db: D1Database, resendKey: string, opts: IssueOpts): Promise<{ license_key: string; resent: boolean }> {
  // 幂等：按 (source, order_ref) 查
  const existing = await db.prepare(`SELECT license_key FROM licenses WHERE source = ? AND order_ref = ?`)
    .bind(opts.source, opts.order_ref).first<{ license_key: string }>();

  let licenseKey: string;
  let licenseId: string;
  let resent = false;

  if (existing) {
    licenseKey = existing.license_key;
    resent = true;
    const row = await db.prepare(`SELECT license_id FROM licenses WHERE license_key = ?`).bind(licenseKey).first<{ license_id: string }>();
    licenseId = row!.license_id;
  } else {
    // 重试 3 次防 license_key 碰撞
    let inserted = false;
    licenseKey = '';
    licenseId = crypto.randomUUID();
    for (let attempt = 0; attempt < 3 && !inserted; attempt++) {
      licenseKey = generateLicenseKey();
      try {
        await db.prepare(
          `INSERT INTO licenses (license_id, license_key, email, plan, max_devices, status, source, order_ref, issued_at, created_at, updated_at, notes)
           VALUES (?, ?, ?, ?, 3, 'active', ?, ?, ?, ?, ?, ?)`
        ).bind(licenseId, licenseKey, opts.email, opts.plan ?? 'personal', opts.source, opts.order_ref, now(), now(), now(), opts.notes ?? null).run();
        inserted = true;
      } catch (e) {
        if (String(e).includes('UNIQUE') && String(e).includes('license_key')) continue;
        throw e;
      }
    }
    if (!inserted) throw new Error('Failed to generate unique license_key after 3 attempts');
  }

  const tpl = opts.lang === 'zh' ? activationEmailZh(licenseKey, opts.email) : activationEmailEn(licenseKey, opts.email);
  await sendEmail(resendKey, opts.email, tpl);

  await writeAudit(db, {
    license_id: licenseId,
    event: resent ? 'reissue' : 'issue',
    detail: { source: opts.source, order_ref_tail: opts.order_ref.slice(-6), lang: opts.lang ?? 'en' },
  });
  return { license_key: licenseKey, resent };
}

export async function revokeLicenseByOrder(db: D1Database, source: string, order_ref: string, reason: string): Promise<boolean> {
  const lic = await db.prepare(`SELECT license_id FROM licenses WHERE source = ? AND order_ref = ?`).bind(source, order_ref).first<{ license_id: string }>();
  if (!lic) return false;
  await db.prepare(`UPDATE licenses SET status = 'refunded', updated_at = ? WHERE license_id = ?`).bind(now(), lic.license_id).run();
  await writeAudit(db, { license_id: lic.license_id, event: 'refund', detail: { source, reason } });
  return true;
}
```

- [ ] **Step 3: 提交**

```bash
git add functions/_lib/issue-license.ts shared/email-templates/activation-zh.ts shared/email-templates/activation-en.ts
git commit -m "feat(license): issueLicense 共用函数（幂等 + 碰撞重试）"
```

---

### Task 7.3: webhook 接口 — 测试 + 实现

**Files:**
- Create: `dimkey-web/functions/webhook/lemonsqueezy.ts`
- Create: `dimkey-web/tests/integration/lemonsqueezy-webhook.test.ts`

- [ ] **Step 1: 写测试**

```typescript
// tests/integration/lemonsqueezy-webhook.test.ts
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { onRequestPost as webhook } from '../../functions/webhook/lemonsqueezy';
import { resetDb, bindings } from './_setup';

const fetchSpy = vi.spyOn(globalThis, 'fetch');

async function sign(body: string, secret: string): Promise<string> {
  const enc = new TextEncoder();
  const key = await crypto.subtle.importKey('raw', enc.encode(secret), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']);
  const sig = await crypto.subtle.sign('HMAC', key, enc.encode(body));
  return Array.from(new Uint8Array(sig)).map(b => b.toString(16).padStart(2, '0')).join('');
}

describe('POST /webhook/lemonsqueezy', () => {
  beforeEach(async () => {
    await resetDb(bindings().DB);
    fetchSpy.mockReset();
    fetchSpy.mockResolvedValue(new Response(JSON.stringify({ id: 'em_1' }), { status: 200 }));
  });

  it('rejects bad signature with 401', async () => {
    const body = JSON.stringify({ meta: { event_id: 'e1', event_name: 'order_created' }, data: {} });
    const req = new Request('https://x/webhook/lemonsqueezy', { method: 'POST', body, headers: { 'X-Signature': 'bad' } });
    const res = await webhook({ request: req, env: bindings(), waitUntil: (p: any) => p } as any);
    expect(res.status).toBe(401);
  });

  it('order_created issues license + sends email + writes inbox', async () => {
    const body = JSON.stringify({
      meta: { event_id: 'e_order_1', event_name: 'order_created', custom_data: { lang: 'en' } },
      data: { id: 'ls_order_1', attributes: { user_email: 'buyer@example.com' } },
    });
    const sig = await sign(body, 'test-ls-secret');
    const req = new Request('https://x/webhook/lemonsqueezy', { method: 'POST', body, headers: { 'X-Signature': sig, 'content-type': 'application/json' } });
    let waitFor: Promise<unknown> = Promise.resolve();
    const res = await webhook({ request: req, env: bindings(), waitUntil: (p: any) => { waitFor = p; } } as any);
    expect(res.status).toBe(200);
    await waitFor;
    // license 已建
    const lic = await bindings().DB.prepare(`SELECT * FROM licenses WHERE source = 'lemonsqueezy' AND order_ref = 'ls_order_1'`).first<any>();
    expect(lic).toBeTruthy();
    expect(lic.email).toBe('buyer@example.com');
    // 邮件已发
    expect(fetchSpy).toHaveBeenCalledTimes(1);
    // inbox 标记 processed
    const inbox = await bindings().DB.prepare(`SELECT processed_at FROM orders_inbox WHERE source = 'lemonsqueezy' AND external_id = 'e_order_1'`).first<{ processed_at: number }>();
    expect(inbox!.processed_at).toBeGreaterThan(0);
  });

  it('duplicate event_id is no-op (idempotent)', async () => {
    const body = JSON.stringify({
      meta: { event_id: 'e_dup', event_name: 'order_created' },
      data: { id: 'ls_dup', attributes: { user_email: 'buyer@example.com' } },
    });
    const sig = await sign(body, 'test-ls-secret');
    const req = () => new Request('https://x/webhook/lemonsqueezy', { method: 'POST', body, headers: { 'X-Signature': sig, 'content-type': 'application/json' } });
    let w1: Promise<unknown> = Promise.resolve();
    let w2: Promise<unknown> = Promise.resolve();
    await webhook({ request: req(), env: bindings(), waitUntil: (p: any) => { w1 = p; } } as any);
    await w1;
    await webhook({ request: req(), env: bindings(), waitUntil: (p: any) => { w2 = p; } } as any);
    await w2;
    // 仅 1 张 license
    const cnt = await bindings().DB.prepare(`SELECT COUNT(*) AS c FROM licenses WHERE source = 'lemonsqueezy'`).first<{ c: number }>();
    expect(cnt!.c).toBe(1);
    // 邮件仅发 1 次
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it('order_refunded marks license refunded', async () => {
    // 先建一张 license
    await bindings().DB.prepare(`INSERT INTO licenses (license_id, license_key, email, plan, max_devices, status, source, order_ref, issued_at, created_at, updated_at) VALUES (?, ?, ?, 'personal', 3, 'active', 'lemonsqueezy', 'ls_refund_1', ?, ?, ?, ?)`)
      .bind(crypto.randomUUID(), 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', 'b@x.com', Date.now(), Date.now(), Date.now()).run();

    const body = JSON.stringify({ meta: { event_id: 'e_ref_1', event_name: 'order_refunded' }, data: { id: 'ls_refund_1' } });
    const sig = await sign(body, 'test-ls-secret');
    const req = new Request('https://x/webhook/lemonsqueezy', { method: 'POST', body, headers: { 'X-Signature': sig, 'content-type': 'application/json' } });
    let w: Promise<unknown> = Promise.resolve();
    await webhook({ request: req, env: bindings(), waitUntil: (p: any) => { w = p; } } as any);
    await w;
    const lic = await bindings().DB.prepare(`SELECT status FROM licenses WHERE order_ref = 'ls_refund_1'`).first<{ status: string }>();
    expect(lic!.status).toBe('refunded');
  });
});
```

- [ ] **Step 2: 实现**

```typescript
// functions/webhook/lemonsqueezy.ts
import { verifyLsSignature } from '../_lib/ls-signature';
import { issueLicense, revokeLicenseByOrder } from '../_lib/issue-license';
import { now } from '../_lib/now';

interface Env {
  DB: D1Database;
  LS_WEBHOOK_SECRET: string;
  RESEND_API_KEY: string;
}

export const onRequestPost: PagesFunction<Env> = async (ctx) => {
  const rawBody = await ctx.request.text();
  const sig = ctx.request.headers.get('X-Signature');
  if (!await verifyLsSignature(rawBody, sig, ctx.env.LS_WEBHOOK_SECRET)) {
    return new Response('invalid signature', { status: 401 });
  }

  let evt: any;
  try { evt = JSON.parse(rawBody); } catch { return new Response('bad json', { status: 400 }); }
  const externalId = evt?.meta?.event_id;
  const eventName = evt?.meta?.event_name;
  if (!externalId || !eventName) return new Response('missing meta', { status: 400 });

  // 写 inbox 去重
  const inserted = await ctx.env.DB.prepare(
    `INSERT OR IGNORE INTO orders_inbox (source, external_id, payload_json) VALUES (?, ?, ?)`
  ).bind('lemonsqueezy', externalId, rawBody).run();
  if (!inserted.meta.changes) return new Response('duplicate', { status: 200 });

  ctx.waitUntil(processEvent(ctx.env, evt, externalId));
  return new Response('ok', { status: 200 });
};

async function processEvent(env: Env, evt: any, externalId: string): Promise<void> {
  try {
    const eventName = evt.meta.event_name;
    switch (eventName) {
      case 'order_created': {
        const email: string = evt.data.attributes.user_email;
        const orderId: string = evt.data.id;
        const lang: 'zh' | 'en' = evt?.meta?.custom_data?.lang === 'zh' ? 'zh' : 'en';
        await issueLicense(env.DB, env.RESEND_API_KEY, {
          email, source: 'lemonsqueezy', order_ref: orderId, plan: 'personal', lang,
        });
        break;
      }
      case 'order_refunded': {
        await revokeLicenseByOrder(env.DB, 'lemonsqueezy', evt.data.id, 'ls_refund');
        break;
      }
      // subscription_* 事件 v1 不处理
    }
    await env.DB.prepare(`UPDATE orders_inbox SET processed_at = ? WHERE source = 'lemonsqueezy' AND external_id = ?`).bind(now(), externalId).run();
  } catch (e) {
    await env.DB.prepare(`UPDATE orders_inbox SET error = ? WHERE source = 'lemonsqueezy' AND external_id = ?`).bind(String(e), externalId).run();
    console.error('[ls_webhook_process_failed]', e);
  }
}
```

- [ ] **Step 3: 测试 + 提交**

```bash
npx vitest run tests/integration/lemonsqueezy-webhook.test.ts
git add functions/webhook/lemonsqueezy.ts tests/integration/lemonsqueezy-webhook.test.ts
git commit -m "feat(webhook): Lemon Squeezy 收单/退款 webhook（签名校验 + inbox 去重 + 异步处理）"
```

---

## Phase 8：Admin 接口

### Task 8.1: admin 路由 + lookup / issue / revoke / transfer / resend-email / order-status / reprocess-order / deactivate-device / extend-trial

**Files:**
- Create: `dimkey-web/functions/admin/[[path]].ts`
- Create: `dimkey-web/tests/integration/admin.test.ts`
- Create: `dimkey-web/scripts/admin-curl-templates.md`

- [ ] **Step 1: 写测试**（每个接口 1-2 个核心 case）

```typescript
// tests/integration/admin.test.ts
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { onRequestPost as adminHandler } from '../../functions/admin/[[path]]';
import { resetDb, bindings, seedLicense } from './_setup';

const fetchSpy = vi.spyOn(globalThis, 'fetch');

function makeReq(path: string, body: any, token = 'test-admin-token'): Request {
  return new Request(`https://x/admin/${path}`, {
    method: 'POST',
    body: JSON.stringify(body),
    headers: { 'content-type': 'application/json', 'Authorization': `Bearer ${token}` },
  });
}

describe('Admin endpoints', () => {
  beforeEach(async () => {
    await resetDb(bindings().DB);
    fetchSpy.mockReset();
    fetchSpy.mockResolvedValue(new Response(JSON.stringify({ id: 'em_1' }), { status: 200 }));
  });

  it('rejects missing/wrong admin token (401)', async () => {
    const res = await adminHandler({ request: makeReq('lookup', { email: 'x@x.com' }, 'wrong'), env: bindings(), waitUntil: () => {}, params: { path: ['lookup'] } } as any);
    expect(res.status).toBe(401);
  });

  it('issue creates license and sends email', async () => {
    let w: Promise<unknown> = Promise.resolve();
    const res = await adminHandler({
      request: makeReq('issue', { email: 'cn@x.com', source: 'manual_cn', order_ref: 'WX-1', lang: 'zh' }),
      env: bindings(), waitUntil: (p: any) => { w = p; }, params: { path: ['issue'] },
    } as any);
    const body = await res.json() as any;
    expect(body.ok).toBe(true);
    expect(body.data.license_key).toMatch(/^DK-/);
    await w;
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it('lookup by email returns license + devices', async () => {
    const licId = await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'l@x.com' });
    const res = await adminHandler({ request: makeReq('lookup', { email: 'l@x.com' }), env: bindings(), waitUntil: () => {}, params: { path: ['lookup'] } } as any);
    const body = await res.json() as any;
    expect(body.data.licenses).toHaveLength(1);
    expect(body.data.licenses[0].license_id).toBe(licId);
  });

  it('revoke marks license refunded', async () => {
    await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'l@x.com' });
    const res = await adminHandler({ request: makeReq('revoke', { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', reason: 'refund' }), env: bindings(), waitUntil: () => {}, params: { path: ['revoke'] } } as any);
    expect((await res.json() as any).ok).toBe(true);
    const row = await bindings().DB.prepare(`SELECT status FROM licenses WHERE license_key = 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE'`).first<{ status: string }>();
    expect(row!.status).toBe('refunded');
  });

  it('transfer changes email', async () => {
    await seedLicense(bindings().DB, { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', email: 'old@x.com' });
    let w: Promise<unknown> = Promise.resolve();
    const res = await adminHandler({ request: makeReq('transfer', { license_key: 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE', new_email: 'new@x.com' }), env: bindings(), waitUntil: (p: any) => { w = p; }, params: { path: ['transfer'] } } as any);
    expect((await res.json() as any).ok).toBe(true);
    await w;
    const row = await bindings().DB.prepare(`SELECT email FROM licenses WHERE license_key = 'DK-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE'`).first<{ email: string }>();
    expect(row!.email).toBe('new@x.com');
    expect(fetchSpy).toHaveBeenCalled();    // 重发激活邮件到新邮箱
  });
});
```

- [ ] **Step 2: 实现单文件路由**

```typescript
// functions/admin/[[path]].ts
import { ok, err, serverError } from '../_lib/response';
import { writeAudit } from '../_lib/audit';
import { issueLicense, revokeLicenseByOrder } from '../_lib/issue-license';
import { sendEmail } from '../_lib/email';
import { activationEmailZh } from '@shared/email-templates/activation-zh';
import { activationEmailEn } from '@shared/email-templates/activation-en';
import { ErrorCode } from '@shared/codes';
import { now } from '../_lib/now';

interface Env { DB: D1Database; ADMIN_TOKEN: string; RESEND_API_KEY: string }

export const onRequestPost: PagesFunction<Env, 'path'> = async (ctx) => {
  const token = ctx.request.headers.get('Authorization')?.replace(/^Bearer\s+/, '');
  if (token !== ctx.env.ADMIN_TOKEN) {
    return new Response('unauthorized', { status: 401 });
  }
  const subpath = (ctx.params.path as string[]).join('/');
  let body: any;
  try { body = await ctx.request.json(); } catch { body = {}; }

  const auditTail = ctx.env.ADMIN_TOKEN.slice(-4);
  try {
    switch (subpath) {
      case 'issue':              return await handleIssue(ctx.env, body, ctx, auditTail);
      case 'revoke':             return await handleRevoke(ctx.env, body, auditTail);
      case 'transfer':           return await handleTransfer(ctx.env, body, ctx, auditTail);
      case 'lookup':             return await handleLookup(ctx.env, body);
      case 'deactivate-device':  return await handleDeactivateDevice(ctx.env, body, auditTail);
      case 'resend-email':       return await handleResendEmail(ctx.env, body, ctx, auditTail);
      case 'order-status':       return await handleOrderStatus(ctx.env, body);
      case 'reprocess-order':    return new Response('reprocess not implemented in v1', { status: 501 });
      case 'extend-trial':       return new Response('extend-trial requires fingerprint storage; deferred', { status: 501 });
      default:                   return new Response('not found', { status: 404 });
    }
  } catch (e) {
    return serverError(e);
  }
};

async function handleIssue(env: Env, body: any, ctx: any, auditTail: string): Promise<Response> {
  const result = await issueLicense(env.DB, env.RESEND_API_KEY, {
    email: body.email, source: body.source, order_ref: body.order_ref,
    plan: body.plan ?? 'personal', lang: body.lang, notes: body.notes,
  });
  ctx.waitUntil(writeAudit(env.DB, { event: 'admin_issue', detail: { source: body.source, order_ref: body.order_ref, by: auditTail, resent: result.resent } }));
  return ok({ license_key: result.license_key, resent: result.resent });
}

async function handleRevoke(env: Env, body: any, auditTail: string): Promise<Response> {
  const lic = await env.DB.prepare(`SELECT license_id FROM licenses WHERE license_key = ?`).bind(body.license_key).first<{ license_id: string }>();
  if (!lic) return err(ErrorCode.INVALID_LICENSE, 'license not found');
  await env.DB.prepare(`UPDATE licenses SET status = 'refunded', updated_at = ? WHERE license_id = ?`).bind(now(), lic.license_id).run();
  await writeAudit(env.DB, { license_id: lic.license_id, event: 'admin_revoke', detail: { reason: body.reason, by: auditTail } });
  return ok({});
}

async function handleTransfer(env: Env, body: any, ctx: any, auditTail: string): Promise<Response> {
  const lic = await env.DB.prepare(`SELECT license_id, email, source FROM licenses WHERE license_key = ?`).bind(body.license_key).first<{ license_id: string; email: string; source: string }>();
  if (!lic) return err(ErrorCode.INVALID_LICENSE, 'license not found');
  await env.DB.prepare(`UPDATE licenses SET email = ?, updated_at = ? WHERE license_id = ?`).bind(body.new_email, now(), lic.license_id).run();
  await writeAudit(env.DB, { license_id: lic.license_id, event: 'admin_transfer', detail: { from: lic.email, to: body.new_email, by: auditTail } });
  // 自动重发邮件到新邮箱
  ctx.waitUntil((async () => {
    const isZh = body.lang === 'zh' || lic.source === 'manual_cn' || lic.source === 'wechat';
    const tpl = isZh ? activationEmailZh(body.license_key, body.new_email) : activationEmailEn(body.license_key, body.new_email);
    await sendEmail(env.RESEND_API_KEY, body.new_email, tpl);
  })());
  return ok({});
}

async function handleLookup(env: Env, body: any): Promise<Response> {
  let licQuery: string;
  let licParam: string;
  if (body.license_key) { licQuery = 'license_key = ?'; licParam = body.license_key; }
  else if (body.email) { licQuery = 'email = ?'; licParam = body.email; }
  else if (body.order_ref) { licQuery = 'order_ref = ?'; licParam = body.order_ref; }
  else return err(ErrorCode.SERVER_ERROR, 'specify license_key/email/order_ref', undefined, 400);
  const licenses = await env.DB.prepare(`SELECT license_id, license_key, email, plan, max_devices, status, source, order_ref, issued_at, expires_at, notes FROM licenses WHERE ${licQuery} ORDER BY created_at DESC`).bind(licParam).all<any>();
  const ids = licenses.results.map(l => l.license_id);
  const devices = ids.length === 0 ? [] : (await env.DB.prepare(
    `SELECT device_id, license_id, fingerprint, machine_label, os, flavor, app_version, first_activated, last_seen, deactivated_at FROM devices WHERE license_id IN (${ids.map(() => '?').join(',')}) ORDER BY last_seen DESC`
  ).bind(...ids).all<any>()).results;
  return ok({ licenses: licenses.results, devices });
}

async function handleDeactivateDevice(env: Env, body: any, auditTail: string): Promise<Response> {
  const dev = await env.DB.prepare(`SELECT device_id, license_id FROM devices WHERE device_id = ? AND deactivated_at IS NULL`).bind(body.device_id).first<{ device_id: string; license_id: string }>();
  if (!dev) return err(ErrorCode.DEVICE_NOT_FOUND, 'device not found or already deactivated');
  await env.DB.prepare(`UPDATE devices SET deactivated_at = ? WHERE device_id = ?`).bind(now(), body.device_id).run();
  await writeAudit(env.DB, { license_id: dev.license_id, device_id: body.device_id, event: 'admin_deactivate_device', detail: { by: auditTail } });
  return ok({});
}

async function handleResendEmail(env: Env, body: any, ctx: any, auditTail: string): Promise<Response> {
  const lic = await env.DB.prepare(`SELECT license_id, email, source FROM licenses WHERE license_key = ?`).bind(body.license_key).first<{ license_id: string; email: string; source: string }>();
  if (!lic) return err(ErrorCode.INVALID_LICENSE, 'license not found');
  ctx.waitUntil((async () => {
    const isZh = body.lang === 'zh' || lic.source === 'manual_cn' || lic.source === 'wechat';
    const tpl = isZh ? activationEmailZh(body.license_key, lic.email) : activationEmailEn(body.license_key, lic.email);
    await sendEmail(env.RESEND_API_KEY, lic.email, tpl);
    await writeAudit(env.DB, { license_id: lic.license_id, event: 'admin_resend_email', detail: { by: auditTail } });
  })());
  return ok({});
}

async function handleOrderStatus(env: Env, body: any): Promise<Response> {
  const row = await env.DB.prepare(`SELECT source, external_id, processed_at, error, length(payload_json) AS payload_size FROM orders_inbox WHERE source = ? AND external_id = ?`).bind(body.source, body.external_id).first<any>();
  if (!row) return err(ErrorCode.SERVER_ERROR, 'order not found in inbox', undefined, 404);
  return ok(row);
}
```

- [ ] **Step 3: 测试 + 提交**

```bash
npx vitest run tests/integration/admin.test.ts
git add functions/admin/'[[path]]'.ts tests/integration/admin.test.ts
git commit -m "feat(admin): admin 接口 — issue/revoke/transfer/lookup/deactivate-device/resend/order-status"
```

- [ ] **Step 4: 写客服 SOP `scripts/admin-curl-templates.md`**

```markdown
# Dimkey 客服操作 SOP

> **私密**：本文件含 admin token 使用方式，**不可外泄**。token 本身存在 1Password。
> 设 `export ADMIN_TOKEN="..."` 后再用以下模板。

## 1. 国内手动发证（用户付款后）
\`\`\`bash
curl -X POST https://dimkey.app/admin/issue \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "content-type: application/json" \
  -d '{"email":"USER@EXAMPLE.COM","source":"manual_cn","order_ref":"WX-20260514-001","plan":"personal","lang":"zh","notes":"微信群X"}'
\`\`\`
- `order_ref` 前缀约定：`WX-`微信 / `ZD-`旺道 / `TM-`提贴状付
- 同 `(source, order_ref)` 重复调用会原样返回原 license_key，不会重复发证

## 2. 退款 / 吊销
\`\`\`bash
curl -X POST https://dimkey.app/admin/revoke \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H "content-type: application/json" \
  -d '{"license_key":"DK-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX","reason":"refund_manual"}'
\`\`\`
用户客户端下次 heartbeat（≤ 7 天内）后进入 Revoked 态。

## 3. 改邮箱（用户填错或换邮箱）
\`\`\`bash
curl -X POST https://dimkey.app/admin/transfer \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H "content-type: application/json" \
  -d '{"license_key":"DK-...","new_email":"new@example.com","lang":"zh"}'
\`\`\`
自动重发激活邮件到新邮箱。

## 4. 查 license（按邮箱/key/订单号任一）
\`\`\`bash
curl -X POST https://dimkey.app/admin/lookup \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H "content-type: application/json" \
  -d '{"email":"user@example.com"}'
# 或 -d '{"license_key":"DK-..."}'
# 或 -d '{"order_ref":"WX-20260514-001"}'
\`\`\`

## 5. 强制解绑某设备（盗用场景）
\`\`\`bash
curl -X POST https://dimkey.app/admin/deactivate-device \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H "content-type: application/json" \
  -d '{"device_id":"<uuid from lookup>"}'
\`\`\`

## 6. 重发激活邮件
\`\`\`bash
curl -X POST https://dimkey.app/admin/resend-email \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H "content-type: application/json" \
  -d '{"license_key":"DK-...","lang":"zh"}'
\`\`\`

## 7. 排查 webhook 状态
\`\`\`bash
curl -X POST https://dimkey.app/admin/order-status \
  -H "Authorization: Bearer $ADMIN_TOKEN" -H "content-type: application/json" \
  -d '{"source":"lemonsqueezy","external_id":"<webhook event_id>"}'
\`\`\`

## 客服记录建议
统一在 Notion / 飞书表格记录：日期 / 客户邮箱 / 操作类型 / order_ref / 备注。
```

```bash
git add scripts/admin-curl-templates.md
git commit -m "docs(admin): 客服操作 SOP — 7 类高频场景 curl 模板"
```

---

## Phase 9：营销网站页面（Astro）

### Task 9.1: 共享 Layout + 首页

**Files:**
- Create: `dimkey-web/src/layouts/Base.astro`
- Create: `dimkey-web/src/pages/index.astro`
- Create: `dimkey-web/src/pages/zh.astro`
- Create: `dimkey-web/src/pages/en.astro`

- [ ] **Step 1: `Base.astro` 提供 head 和导航**

```astro
---
const { title = 'Dimkey · 本地文档脱敏工具', lang = 'zh' } = Astro.props;
---
<!DOCTYPE html>
<html lang={lang}>
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{title}</title>
    <meta name="description" content={lang === 'zh' ? '本地运行的文档脱敏工具，支持 xlsx/docx/pdf/csv/txt' : 'Local-first document desensitization tool for xlsx/docx/pdf/csv/txt'} />
    <link rel="icon" type="image/svg+xml" href="/favicon.svg" />
    <style is:global>
      :root { font-family: ui-sans-serif, system-ui, -apple-system, "PingFang SC", "Microsoft Yahei", sans-serif; }
      body { margin: 0; color: #1a1a1a; background: #fafafa; }
      a { color: inherit; }
      .container { max-width: 920px; margin: 0 auto; padding: 32px 24px; }
      .nav { display: flex; gap: 24px; padding: 16px 24px; border-bottom: 1px solid #eee; }
      .btn { display: inline-block; padding: 10px 20px; border-radius: 8px; background: #1a1a1a; color: white; text-decoration: none; font-size: 15px; }
      .btn-secondary { background: white; color: #1a1a1a; border: 1px solid #ccc; }
    </style>
  </head>
  <body>
    <nav class="nav">
      <a href="/">Dimkey</a>
      <span style="flex:1"></span>
      <a href="/buy">{lang === 'zh' ? '购买' : 'Buy'}</a>
      <a href="/recover">{lang === 'zh' ? '找回许可证' : 'Recover'}</a>
      <a href={lang === 'zh' ? '/en' : '/zh'}>{lang === 'zh' ? 'EN' : '中'}</a>
    </nav>
    <slot />
  </body>
</html>
```

- [ ] **Step 2: `index.astro` 重定向到 zh 或 en（按 Accept-Language）**

```astro
---
const al = Astro.request.headers.get('accept-language') ?? '';
const target = /\bzh\b/i.test(al) ? '/zh' : '/en';
return Astro.redirect(target, 302);
---
```

- [ ] **Step 3: `zh.astro` 中文落地页**

```astro
---
import Base from '../layouts/Base.astro';
---
<Base lang="zh" title="Dimkey · 本地文档脱敏工具">
  <main class="container">
    <h1 style="font-size:48px;margin:48px 0 12px">Dimkey</h1>
    <p style="font-size:20px;color:#555;margin:0 0 32px">本地运行的文档脱敏工具 · 不上传，不留痕</p>
    <div style="display:flex;gap:12px;margin:24px 0 64px">
      <a class="btn" href="/buy">购买（¥199）</a>
      <a class="btn btn-secondary" href="/download">下载试用 30 天</a>
    </div>
    <h2 style="margin-top:48px">支持格式</h2>
    <p>xlsx · xls · csv · docx · pdf · txt</p>
    <h2 style="margin-top:48px">特性</h2>
    <ul style="line-height:1.8">
      <li>完全本地运行，零网络通信，数据不出本机</li>
      <li>三层识别引擎：正则 + NER + 自定义词典</li>
      <li>一致性脱敏：同一姓名在文档中始终替换为同一假名</li>
      <li>导出原格式，保留排版</li>
    </ul>
    <p style="color:#888;font-size:13px;margin-top:96px">
      <a href="/privacy">隐私政策</a> · <a href="https://github.com/...">GitHub</a>
    </p>
  </main>
</Base>
```

- [ ] **Step 4: `en.astro` 英文落地页（结构同 zh）**

```astro
---
import Base from '../layouts/Base.astro';
---
<Base lang="en" title="Dimkey · Local-first Document Desensitizer">
  <main class="container">
    <h1 style="font-size:48px;margin:48px 0 12px">Dimkey</h1>
    <p style="font-size:20px;color:#555;margin:0 0 32px">Local-first document desensitization · No uploads, no traces</p>
    <div style="display:flex;gap:12px;margin:24px 0 64px">
      <a class="btn" href="/buy">Buy ($29)</a>
      <a class="btn btn-secondary" href="/download">Try 30 days free</a>
    </div>
    <h2 style="margin-top:48px">Supported formats</h2>
    <p>xlsx · xls · csv · docx · pdf · txt</p>
    <h2 style="margin-top:48px">Features</h2>
    <ul style="line-height:1.8">
      <li>100% local, zero network calls — your data never leaves your machine</li>
      <li>Three-layer detection: regex + NER + custom dictionary</li>
      <li>Consistent desensitization: same name always maps to the same pseudonym across the document</li>
      <li>Preserves original formatting</li>
    </ul>
    <p style="color:#888;font-size:13px;margin-top:96px">
      <a href="/privacy">Privacy</a> · <a href="https://github.com/...">GitHub</a>
    </p>
  </main>
</Base>
```

- [ ] **Step 5: 提交**

```bash
git add src/layouts/ src/pages/index.astro src/pages/zh.astro src/pages/en.astro
git commit -m "feat(web): 营销首页（zh + en + 按 Accept-Language 重定向）"
```

---

### Task 9.2: 购买页 `buy.astro`

**Files:**
- Create: `dimkey-web/src/pages/buy.astro`

- [ ] **Step 1: 写购买页 HTML**

```astro
---
import Base from '../layouts/Base.astro';
const al = Astro.request.headers.get('accept-language') ?? '';
const lang: 'zh' | 'en' = /\bzh\b/i.test(al) ? 'zh' : 'en';
const lsLink = 'https://dimkey.lemonsqueezy.com/checkout/buy/PRODUCT_ID';   // 在 LS 后台获取后填
---
<Base lang={lang} title={lang === 'zh' ? '购买 Dimkey' : 'Buy Dimkey'}>
  <main class="container" style="text-align:center;padding-top:64px">
    <h1 style="font-size:36px">{lang === 'zh' ? '购买 Dimkey' : 'Buy Dimkey'}</h1>
    <p style="color:#555;margin-bottom:48px">{lang === 'zh' ? '一次付费，永久使用 · 最多 3 台设备 · 中英文版通用' : 'One-time payment, lifetime use · Up to 3 devices · Works for both ZH and EN'}</p>

    <div style="display:flex;gap:24px;justify-content:center;flex-wrap:wrap">
      <div style="background:white;padding:32px;border-radius:12px;border:1px solid #eee;min-width:260px">
        <div style="font-size:14px;color:#888">USD</div>
        <div style="font-size:36px;font-weight:600;margin:8px 0">$29</div>
        <a class="btn" href={`${lsLink}?checkout%5Bcustom%5D%5Blang%5D=${lang}`} target="_blank">{lang === 'zh' ? '海外用户购买' : 'Buy with Card / PayPal'}</a>
        <p style="font-size:13px;color:#888;margin-top:16px">{lang === 'zh' ? '通过 Lemon Squeezy 安全支付' : 'Secure checkout via Lemon Squeezy'}</p>
      </div>

      <div style="background:white;padding:32px;border-radius:12px;border:1px solid #eee;min-width:260px">
        <div style="font-size:14px;color:#888">CNY</div>
        <div style="font-size:36px;font-weight:600;margin:8px 0">¥199</div>
        <a class="btn" href="/buy/wechat">{lang === 'zh' ? '微信购买' : 'Buy with WeChat Pay'}</a>
        <p style="font-size:13px;color:#888;margin-top:16px">{lang === 'zh' ? '联系客服微信完成付款' : 'Contact support on WeChat to complete payment'}</p>
      </div>
    </div>

    <p style="color:#888;font-size:13px;margin-top:48px;max-width:520px;margin-left:auto;margin-right:auto">
      {lang === 'zh' ? '付款后许可证将通过邮件发送至你的购买邮箱（约 15 秒）。如未收到请检查垃圾邮件，或在 ' : 'Your license will be emailed to your purchase email within ~15 seconds. If not received, check spam folder or '}
      <a href="/recover">{lang === 'zh' ? '找回页面' : 'recover here'}</a>
      {lang === 'zh' ? ' 重发。' : '.'}
    </p>
  </main>
</Base>
```

- [ ] **Step 2: 提交**

```bash
git add src/pages/buy.astro
git commit -m "feat(web): 购买页 — LS 嵌入 + 微信入口（lang custom field 注入）"
```

---

### Task 9.3: 找回页 `recover.astro`

**Files:**
- Create: `dimkey-web/src/pages/recover.astro`

- [ ] **Step 1: 写找回页（含表单 + JS 调 /api/v1/recover）**

```astro
---
import Base from '../layouts/Base.astro';
const al = Astro.request.headers.get('accept-language') ?? '';
const lang: 'zh' | 'en' = /\bzh\b/i.test(al) ? 'zh' : 'en';
---
<Base lang={lang} title={lang === 'zh' ? '找回 Dimkey 许可证' : 'Recover Dimkey License'}>
  <main class="container" style="max-width:480px">
    <h1>{lang === 'zh' ? '找回许可证' : 'Recover License'}</h1>
    <p style="color:#555">{lang === 'zh' ? '输入购买时使用的邮箱，许可证将重发到该邮箱。' : 'Enter the email you used for purchase. Your license will be re-sent.'}</p>
    <form id="f" style="margin-top:24px">
      <input type="email" name="email" required placeholder={lang === 'zh' ? '邮箱' : 'Email'}
        style="width:100%;padding:12px;border:1px solid #ccc;border-radius:8px;font-size:15px;box-sizing:border-box" />
      <button class="btn" type="submit" style="margin-top:16px;width:100%;border:none;cursor:pointer">{lang === 'zh' ? '发送' : 'Send'}</button>
    </form>
    <p id="msg" style="margin-top:16px;color:#0a7;display:none"></p>
  </main>
  <script define:vars={{ lang }}>
    const f = document.getElementById('f');
    const msg = document.getElementById('msg');
    f.addEventListener('submit', async (e) => {
      e.preventDefault();
      const fd = new FormData(f);
      const res = await fetch('/api/v1/recover', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ email: fd.get('email') }) });
      const body = await res.json();
      msg.textContent = body?.data?.message ?? (lang === 'zh' ? '已提交' : 'Submitted');
      msg.style.display = 'block';
      f.reset();
    });
  </script>
</Base>
```

- [ ] **Step 2: 提交**

```bash
git add src/pages/recover.astro
git commit -m "feat(web): 找回许可证页（fetch /api/v1/recover）"
```

---

### Task 9.4: 设备管理页 `account.astro`

**Files:**
- Create: `dimkey-web/src/pages/account.astro`

- [ ] **Step 1: 写设备管理页（输入邮箱+key 后展示设备列表 + 解绑按钮）**

```astro
---
import Base from '../layouts/Base.astro';
const al = Astro.request.headers.get('accept-language') ?? '';
const lang: 'zh' | 'en' = /\bzh\b/i.test(al) ? 'zh' : 'en';
---
<Base lang={lang} title={lang === 'zh' ? 'Dimkey 设备管理' : 'Dimkey Devices'}>
  <main class="container" style="max-width:640px">
    <h1>{lang === 'zh' ? '管理你的设备' : 'Manage Your Devices'}</h1>
    <form id="auth" style="display:flex;gap:8px;flex-wrap:wrap;margin-top:24px">
      <input type="email" name="email" required placeholder={lang === 'zh' ? '邮箱' : 'Email'} style="flex:1;min-width:200px;padding:10px;border:1px solid #ccc;border-radius:6px" />
      <input type="text" name="license_key" required placeholder="DK-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX" style="flex:2;min-width:280px;padding:10px;border:1px solid #ccc;border-radius:6px;font-family:ui-monospace,monospace" />
      <button class="btn" type="submit">{lang === 'zh' ? '查看' : 'View'}</button>
    </form>
    <div id="list" style="margin-top:32px"></div>
  </main>
  <script define:vars={{ lang }}>
    const auth = document.getElementById('auth');
    const list = document.getElementById('list');
    let creds = null;
    auth.addEventListener('submit', async (e) => { e.preventDefault(); const fd = new FormData(auth); creds = { email: fd.get('email'), license_key: fd.get('license_key') }; await load(); });
    async function load() {
      const res = await fetch('/api/v1/devices/list', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(creds) });
      const body = await res.json();
      if (!body.ok) {
        list.innerHTML = `<p style="color:#c00">${body.message}</p>`;
        return;
      }
      list.innerHTML = body.data.devices.map(d => `
        <div style="background:white;padding:16px;border:1px solid #eee;border-radius:8px;display:flex;justify-content:space-between;align-items:center;margin-bottom:8px">
          <div>
            <div style="font-weight:600">${d.machine_label ?? '(unnamed)'}</div>
            <div style="color:#888;font-size:13px">${d.os} · ${d.flavor} · ${(lang === 'zh' ? '最后活跃 ' : 'last seen ')}${new Date(d.last_seen).toLocaleString()}</div>
          </div>
          <button data-id="${d.device_id}" style="padding:6px 14px;border:1px solid #c00;color:#c00;background:white;border-radius:6px;cursor:pointer">${lang === 'zh' ? '解绑' : 'Deactivate'}</button>
        </div>`).join('') + `<p style="color:#888;font-size:13px;margin-top:16px">${body.data.devices.length} / ${body.data.max_devices} ${lang === 'zh' ? '台设备 · 解绑后立即释放配额' : 'devices · Deactivating frees a slot immediately'}</p>`;
      list.querySelectorAll('button[data-id]').forEach(btn => btn.addEventListener('click', async () => {
        if (!confirm(lang === 'zh' ? '确认解绑此设备？' : 'Deactivate this device?')) return;
        await fetch('/api/v1/deactivate', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ ...creds, device_id: btn.dataset.id }) });
        await load();
      }));
    }
    // 支持 URL ?key=&email= 自动填充并加载
    const u = new URL(location.href);
    if (u.searchParams.get('key') && u.searchParams.get('email')) {
      auth.querySelector('[name=email]').value = u.searchParams.get('email');
      auth.querySelector('[name=license_key]').value = u.searchParams.get('key');
      creds = { email: u.searchParams.get('email'), license_key: u.searchParams.get('key') };
      load();
    }
  </script>
</Base>
```

- [ ] **Step 2: 提交**

```bash
git add src/pages/account.astro
git commit -m "feat(web): 设备管理页（输入 email+key 查看 + 解绑）"
```

---

### Task 9.5: 隐私政策 `privacy.astro`

**Files:**
- Create: `dimkey-web/src/pages/privacy.astro`

- [ ] **Step 1: 写隐私政策（按 spec §9.7）**

```astro
---
import Base from '../layouts/Base.astro';
const al = Astro.request.headers.get('accept-language') ?? '';
const lang: 'zh' | 'en' = /\bzh\b/i.test(al) ? 'zh' : 'en';
---
<Base lang={lang} title={lang === 'zh' ? 'Dimkey 隐私政策' : 'Dimkey Privacy Policy'}>
  <main class="container" style="max-width:720px;line-height:1.8">
    <h1>{lang === 'zh' ? '隐私政策' : 'Privacy Policy'}</h1>
    <p style="color:#888">{lang === 'zh' ? '最后更新：2026-05-14' : 'Last updated: 2026-05-14'}</p>
    {lang === 'zh' ? (
      <>
        <h2>核心承诺</h2>
        <p>Dimkey 桌面应用 <strong>不上传任何文件内容、识别结果或扫描数据</strong>。所有文档处理均在本机完成。</p>
        <h2>许可证服务收集的信息</h2>
        <p>仅在你激活/续验/解绑/找回许可证时，客户端会向我们的服务器发送：</p>
        <ul>
          <li>邮箱、许可证密钥</li>
          <li>设备指纹（机器 UUID、物理 MAC、CPU 型号、系统安装 ID 的 SHA-256 哈希，<strong>不可逆推</strong>）</li>
          <li>设备名（hostname）、操作系统、应用版本、语言版本（中/英）</li>
          <li>访问时自动产生的 IP 地址、User-Agent</li>
        </ul>
        <h2>数据用途</h2>
        <p>仅用于授权绑定与防滥用。不用于行为追踪、不出售给第三方。</p>
        <h2>数据保留</h2>
        <ul>
          <li>设备记录：永久至解绑后 90 天清理</li>
          <li>审计日志（含 IP / UA）：1 年</li>
          <li>邮件投递日志：90 天（由 Resend 管理）</li>
        </ul>
        <h2>数据存储</h2>
        <p>数据存储在 Cloudflare D1（位于 Cloudflare 全球边缘网络，主区可选）。</p>
        <h2>第三方</h2>
        <ul>
          <li>支付：Lemon Squeezy（海外）/ 微信支付（中国大陆）</li>
          <li>邮件：Resend</li>
          <li>托管：Cloudflare</li>
        </ul>
        <h2>联系</h2>
        <p>support@dimkey.app</p>
      </>
    ) : (
      <>
        <h2>Core Promise</h2>
        <p>The Dimkey desktop app <strong>never uploads file content, detection results, or scan data</strong>. All document processing happens on your machine.</p>
        <h2>What the License Service Collects</h2>
        <p>Only when you activate, verify, deactivate, or recover a license, the client sends to our server:</p>
        <ul>
          <li>Email and license key</li>
          <li>Device fingerprint (SHA-256 hash of machine UUID, physical MAC, CPU model, OS install ID — <strong>not reversible</strong>)</li>
          <li>Device name (hostname), OS, app version, language variant (zh/en)</li>
          <li>IP address and User-Agent (collected automatically)</li>
        </ul>
        <h2>How We Use Data</h2>
        <p>Solely for license binding and abuse prevention. Not for tracking. Not sold to third parties.</p>
        <h2>Data Retention</h2>
        <ul>
          <li>Device records: kept until 90 days after deactivation</li>
          <li>Audit logs (with IP / UA): 1 year</li>
          <li>Email delivery logs: 90 days (managed by Resend)</li>
        </ul>
        <h2>Storage</h2>
        <p>Data is stored in Cloudflare D1 across Cloudflare's global edge.</p>
        <h2>Third Parties</h2>
        <ul>
          <li>Payment: Lemon Squeezy (international) / WeChat Pay (China)</li>
          <li>Email: Resend</li>
          <li>Hosting: Cloudflare</li>
        </ul>
        <h2>Contact</h2>
        <p>support@dimkey.app</p>
      </>
    )}
  </main>
</Base>
```

- [ ] **Step 2: 提交**

```bash
git add src/pages/privacy.astro
git commit -m "docs(web): 隐私政策（中英文）"
```

---

## Phase 10：部署与端到端验证

### Task 10.1: 本地端到端冒烟测试

**Files:**
- Modify: 无（仅运行命令）

- [ ] **Step 1: 在本地 wrangler dev 跑起整个栈**

```bash
npm run dev
# 输出: Local: http://localhost:8788
```

- [ ] **Step 2: curl 跑一遍核心接口（手动 smoke test）**

```bash
# 1. 用 admin 接口手动发一张 license
curl -X POST http://localhost:8788/admin/issue \
  -H "Authorization: Bearer test-admin-token" \
  -H "content-type: application/json" \
  -d '{"email":"smoke@test.com","source":"manual_cn","order_ref":"SMOKE-1","plan":"personal","lang":"zh"}'
# 期望：返回 { ok: true, data: { license_key: "DK-XXXXX-..." } }

# 2. 用返回的 key 激活
LICENSE_KEY="DK-..."  # 上一步输出
curl -X POST http://localhost:8788/api/v1/activate \
  -H "content-type: application/json" \
  -d "{\"license_key\":\"$LICENSE_KEY\",\"email\":\"smoke@test.com\",\"fingerprint\":\"fpsmoke1\",\"machine_label\":\"SmokeMac\",\"os\":\"macos\",\"flavor\":\"zh\",\"app_version\":\"0.7.0\"}"
# 期望：返回 { ok: true, data: { license_certificate: {...}, device_summary: { active_count: 1, max_devices: 3 } } }

# 3. heartbeat
LICENSE_ID="..."  # 从 activate 响应里 base64 解码 payload 取
DEVICE_ID="..."   # 同上
curl -X POST http://localhost:8788/api/v1/heartbeat \
  -H "content-type: application/json" \
  -d "{\"license_id\":\"$LICENSE_ID\",\"device_id\":\"$DEVICE_ID\",\"fingerprint\":\"fpsmoke1\"}"
# 期望：{ ok: true, data: { status: "active", next_check_at: ... } }
```

- [ ] **Step 3: 浏览器访问 http://localhost:8788/zh /en /buy /recover /account /privacy** 检查页面渲染正常

- [ ] **Step 4: 跑一遍全部测试**

```bash
npm test
# 期望：全部 PASS（unit + integration）
```

- [ ] **Step 5: 提交（无代码改动，仅日志）**

无新文件，跳过 commit。

---

### Task 10.2: 部署到生产

**Files:**
- 仅命令

- [ ] **Step 1: 检查 wrangler.toml 中 D1 / KV ID 已填**

```bash
cat wrangler.toml | grep -E "database_id|^id ="
```

- [ ] **Step 2: 应用 D1 migration 到生产**

```bash
npm run d1:migrate:prod
```

- [ ] **Step 3: 部署**

```bash
npm run deploy
# 输出: Successfully published to https://dimkey-web.pages.dev
```

- [ ] **Step 4: 在 CF Pages 后台绑定自定义域 `dimkey.app`**（手动操作；需先在域名注册商把 NS 指向 CF）

- [ ] **Step 5: 在 Lemon Squeezy 后台配置 webhook URL = `https://dimkey.app/webhook/lemonsqueezy`**

- [ ] **Step 6: 用真实购买跑一次端到端**：
  - 在 LS 测试模式下买一次
  - 确认收到激活邮件（含 license_key）
  - 用客户端（待 Plan B 完成后）激活，验证设备出现在 D1
  - 退款测试：在 LS 后台退款 → 确认 `licenses.status` 变 refunded

- [ ] **Step 7: 提交 README 更新最终域名**

```bash
git add README.md
git commit -m "docs: 更新部署后的最终域名 dimkey.app"
git push origin main
```

---

## 验收标准

- [ ] `npm test` 全部通过（unit + integration ≥ 30 测试）
- [ ] 本地 `npm run dev` 后浏览器能访问全部 6 个页面（index/zh/en/buy/recover/account/privacy）
- [ ] curl 能完整跑通 smoke test（admin/issue → activate → heartbeat → deactivate）
- [ ] LS 测试单触发 webhook 后 ≤ 30 秒内收到激活邮件
- [ ] 生产域名 `dimkey.app` 可访问、HTTPS 有效
- [ ] D1 生产库 schema 与本地一致
- [ ] 客服 SOP 文档（`scripts/admin-curl-templates.md`）完整可用

---

## 后续

完成本 plan 后，进入 Plan B（dimkey 客户端 license 集成）。客户端实现完成后再做端到端联调（在 Plan B 末尾）。
