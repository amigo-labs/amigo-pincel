// Compile-time drift check between the hand-written command types in wasm.ts
// and the ts-rs-generated mirror of the Rust `CommandSpec` enum (ADR-014).
//
// serde fills defaulted fields silently, so a misspelled discriminant or
// field name would otherwise surface only as wrong runtime behavior. These
// type-level assertions make `pnpm check` fail instead. Regenerate the mirror
// with `cargo test -p fineliner-wasm export_command_spec`.
import type { CommandSpec } from './generated/CommandSpec';
import type { EffectSpec } from './generated/EffectSpec';
import type { EffectCommand, ToolCommand } from './wasm';

type AssertNever<T extends never> = T;

type HandTag = ToolCommand['type'];
type GenTag = CommandSpec['type'];
type HandOf<K extends HandTag> = Extract<ToolCommand, { type: K }>;
type GenOf<K extends GenTag> = Extract<CommandSpec, { type: K }>;

/** Every Rust command variant must be mirrored in wasm.ts. */
export type _MissingCommands = AssertNever<Exclude<GenTag, HandTag>>;

/** Every hand-written command must exist in Rust (typo in `type` = dead command). */
export type _UnknownCommands = AssertNever<Exclude<HandTag, GenTag>>;

/** A field name Rust does not know is silently dropped by serde — reject it. */
export type _UnknownFields = AssertNever<
  { [K in HandTag & GenTag]: Exclude<keyof HandOf<K>, keyof GenOf<K>> }[HandTag & GenTag]
>;

/**
 * Field types must be assignable to the generated ones. `Partial` because
 * serde defaults make omitting a field legal at runtime.
 */
export type _FieldTypeMismatches = AssertNever<
  { [K in HandTag & GenTag]: HandOf<K> extends Partial<GenOf<K>> ? never : K }[HandTag & GenTag]
>;

// The same drift assertions for EffectCommand against the Rust EffectSpec.
type EffHandTag = EffectCommand['type'];
type EffGenTag = EffectSpec['type'];
type EffHandOf<K extends EffHandTag> = Extract<EffectCommand, { type: K }>;
type EffGenOf<K extends EffGenTag> = Extract<EffectSpec, { type: K }>;

/** Every Rust effect variant must be mirrored in wasm.ts. */
export type _MissingEffects = AssertNever<Exclude<EffGenTag, EffHandTag>>;

/** Every hand-written effect must exist in Rust. */
export type _UnknownEffects = AssertNever<Exclude<EffHandTag, EffGenTag>>;

/** A field name Rust does not know is silently dropped by serde — reject it. */
export type _UnknownEffectFields = AssertNever<
  { [K in EffHandTag & EffGenTag]: Exclude<keyof EffHandOf<K>, keyof EffGenOf<K>> }[EffHandTag &
    EffGenTag]
>;

/** Effect field types must be assignable to the generated ones. */
export type _EffectFieldTypeMismatches = AssertNever<
  {
    [K in EffHandTag & EffGenTag]: EffHandOf<K> extends Partial<EffGenOf<K>> ? never : K;
  }[EffHandTag & EffGenTag]
>;
