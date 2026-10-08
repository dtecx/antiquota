/** Groups concrete model IDs into families without inventing available variants. */
import type { Model } from '../gen/Model';
export function modelFamilies(models: Model[], selected: string) {
  const rows = models.some((m) => m.id === selected)
    ? models
    : [...models, { id: selected, label: selected }];
  const families = new Map<string, { name: string; variants: { id: string; effort: string }[] }>();
  for (const row of rows) {
    const match = row.label.match(/^(.*?)\s*\(([^)]+)\)\s*$/);
    const name = match?.[1]?.trim() ?? row.label;
    const family = families.get(name) ?? { name, variants: [] };
    family.variants.push({ id: row.id, effort: match?.[2]?.toLowerCase() ?? '' });
    families.set(name, family);
  }
  return [...families.values()];
}
export function pickFamily(models: Model[], selected: string, name: string) {
  const families = modelFamilies(models, selected);
  const previous = families.flatMap((f) => f.variants).find((v) => v.id === selected)?.effort;
  const variants = families.find((f) => f.name === name)?.variants ?? [];
  return (
    (
      variants.find((v) => v.effort === previous) ??
      variants.find((v) => v.effort === 'high') ??
      variants[0]
    )?.id ?? selected
  );
}
