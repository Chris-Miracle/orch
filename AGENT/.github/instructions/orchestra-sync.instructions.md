---
applyTo: "**/*sync*.rs,**/*render*.rs,**/templates/**"
---

Maintain strict sync guarantees.

- Render from registry only.
- Compute hash before write.
- Skip write when hash is unchanged.
- Keep dry-run output faithful to real sync.

