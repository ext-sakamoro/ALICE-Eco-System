# ALICE Brand Guidelines

**Effective Date:** 2026-02-24
**Official Reference:** https://alicelaw.net

## 1. Trademarks

The following names and marks are trademarks of Moroya Sakamoto / Extoria:

- **ALICE** (when used in the context of software infrastructure)
- **Project A.L.I.C.E.** (Adaptive Lightweight Infrastructure for Compute Everywhere)
- **ALICE-\*** (the naming pattern for all ecosystem crates, e.g., ALICE-Auth, ALICE-Physics, ALICE-SDF)
- **"Powered by ALICE"** (badge and attribution mark)

These marks are not registered trademarks at this time. Trademark rights are established through continuous use in commerce since 2024.

## 2. Permitted Use

You **may** use the ALICE name when:

- Referring to the official ALICE project in documentation, blog posts, or academic papers.
- Stating compatibility or interoperability (e.g., "Compatible with ALICE-Physics").
- Using ALICE crates as dependencies in your project, as permitted by each crate's license (MIT, AGPL-3.0, etc.).
- Attributing the origin of code forked from ALICE repositories.

## 3. Prohibited Use

You **may not** use the ALICE name when:

- Naming a competing product or service in a way that implies official endorsement or origin (e.g., "ALICE Cloud Platform" by a third party).
- Creating a fork and continuing to use the ALICE name without clearly distinguishing it as unofficial (e.g., acceptable: "MyFork, based on ALICE-Physics"; unacceptable: "ALICE-Physics Pro").
- Using the ALICE name in domain names, app store listings, or SaaS product names without written permission.
- Implying that your product is created, maintained, or endorsed by the ALICE project.

## 4. Fork Naming Policy

If you fork any ALICE crate:

1. **Remove or replace** the "ALICE" prefix in your fork's name.
2. **Add a clear notice** in your README that your project is derived from ALICE but is not an official ALICE product.
3. **Retain** all copyright notices and license text as required by the applicable license.

Example acceptable fork name: `quantum-physics-engine (forked from ALICE-Physics)`
Example unacceptable fork name: `ALICE-Physics-Enterprise`

## 5. SaaS Wrapping Policy

Using ALICE crates as a backend for a SaaS product:

- **AGPL-3.0 crates** (Auth, Physics, ML, Codec, Sync, Crypto, CDN): You must release your complete source code under AGPL-3.0, including all modifications and server-side code that uses these crates.
- **MIT crates** (SDF, Eco-System): You may use these freely, but you may not name your SaaS product using the ALICE trademark.
- **Commercial license**: Contact us at https://alicelaw.net for commercial licensing that permits proprietary use without AGPL obligations.

## 6. "Powered by ALICE" Attribution

If you use ALICE crates in your product and wish to display attribution:

```
Powered by ALICE — https://alicelaw.net
```

Use of the "Powered by ALICE" badge is encouraged but not required for MIT-licensed crates. For AGPL-licensed crates, proper license compliance (source disclosure) is mandatory regardless of badge display.

## 7. Logo and Visual Identity

The ALICE project does not currently distribute official logo files. Any future visual assets will be published at https://alicelaw.net/brand.

## 8. Reporting Violations

If you believe someone is misusing the ALICE trademarks or violating the license terms:

- **Report:** https://alicelaw.net/report
- **Email:** Contact via the form at https://alicelaw.net

## 9. Contact for Commercial Licensing

For commercial licensing inquiries, enterprise support, or trademark usage permissions:

- **Website:** https://alicelaw.net
- **Repository:** https://github.com/ext-sakamoro

---

Copyright 2024-2026 Moroya Sakamoto / Extoria. All rights reserved.
