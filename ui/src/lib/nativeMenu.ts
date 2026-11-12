// The system's own context menu, for windows too small to hold an in-page menu (the floating
// strip): it opens outside the window, at the cursor, like any desktop menu. Takes the same
// item list as `openMenu`, so a menu is written once.
import { CheckMenuItem, Menu, MenuItem as NativeItem, PredefinedMenuItem } from '@tauri-apps/api/menu'
import type { MenuItem } from './state.svelte'

/** Drops separators at the ends and repeated ones (left behind by conditional items). */
function tidy(items: MenuItem[]): MenuItem[] {
  const out: MenuItem[] = []
  for (const i of items) {
    if (i.sep && (!out.length || out[out.length - 1].sep)) continue
    out.push(i)
  }
  while (out.length && out[out.length - 1].sep) out.pop()
  return out
}

export async function popupNative(items: MenuItem[]) {
  const built = await Promise.all(
    tidy(items).map((i) => {
      if (i.sep) return PredefinedMenuItem.new({ item: 'Separator' })
      const base = { text: i.label ?? '', enabled: !i.disabled, action: () => void i.action?.() }
      return i.checked === undefined ? NativeItem.new(base) : CheckMenuItem.new({ ...base, checked: i.checked })
    }),
  )
  const menu = await Menu.new({ items: built })
  await menu.popup()
}
