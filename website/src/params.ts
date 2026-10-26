import { defineParams } from '@sveltejs/kit/params';

// `[[lang=locale]]` matches the non-default site languages only; English
// lives at the root (website spec §11).
const matchLocale = (param: string) => param === 'de';

export const params = defineParams({
  locale: (param) => (matchLocale(param) ? param : undefined),
});
