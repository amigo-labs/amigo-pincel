// `[[lang=locale]]` matches the non-default site languages only; English
// lives at the root (website spec §11).
import type { ParamMatcher } from '@sveltejs/kit';

export const match: ParamMatcher = (param) => param === 'de';
