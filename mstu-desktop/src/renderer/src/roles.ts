import type { Library } from './types'

/// What a plugin does to data: the one thing colour says across the app.
export type Role = 'source' | 'transform' | 'sink'

export const ROLES: Role[] = ['source', 'transform', 'sink']

export const ROLE_NAMES: Record<Role, string> = {
  source: 'Sources',
  transform: 'Transforms',
  sink: 'Sinks'
}

export function role(library: Library | undefined | null): Role {
  if (library?.source || !library?.input) return 'source'
  if (!library?.output) return 'sink'

  return 'transform'
}
