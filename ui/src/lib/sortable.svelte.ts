// Pointer-driven reordering shared by the card grid and the floating launcher.
//
// Press and move ~5 px to start: a lifted copy of the item (the ghost) follows the pointer, the
// item's own slot stays in place as a dashed placeholder, and the other items slide to their new
// places as the pointer passes over them (the list renders `order`, keyed `animate:flip` does the
// sliding). On release the ghost glides into the placeholder and the new order is committed.
// Esc cancels. A click that ends a drag is swallowed so it does not also launch the item.

export interface SortableOptions {
  /** Ids in their current visible order. */
  ids: () => string[]
  /** Called with the new visible order when it changed. */
  commit: (ids: string[]) => void
  enabled: () => boolean
  /** Container scrolled while dragging near its edges. */
  scroller?: () => HTMLElement | null
}

const THRESHOLD = 5
const GLIDE_MS = 190

export function createSortable(opts: SortableOptions) {
  const st = $state({ dragging: null as string | null, order: null as string[] | null })

  let source: HTMLElement | null = null
  let ghost: HTMLElement | null = null
  let id = ''
  let sx = 0
  let sy = 0
  let origin = { left: 0, top: 0 }
  let started = false
  let lastX = 0
  let lastY = 0
  let scrollTimer = 0

  function reduced() {
    return document.documentElement.dataset.motion === 'reduce' || matchMedia('(prefers-reduced-motion: reduce)').matches
  }

  function down(e: PointerEvent, itemId: string) {
    if (!opts.enabled() || e.button !== 0) return
    // Controls inside the item keep their own clicks; the item itself may be a button.
    const control = (e.target as HTMLElement).closest('button, input, select, textarea, a, [data-no-drag]')
    if (control && control !== e.currentTarget) return
    source = e.currentTarget as HTMLElement
    id = itemId
    sx = lastX = e.clientX
    sy = lastY = e.clientY
    started = false
    addEventListener('pointermove', move)
    addEventListener('pointerup', up)
    addEventListener('pointercancel', cancel)
    addEventListener('keydown', key)
  }

  function begin() {
    if (!source) return
    started = true
    const r = source.getBoundingClientRect()
    origin = { left: r.left, top: r.top }
    ghost = source.cloneNode(true) as HTMLElement
    ghost.classList.add('sort-ghost')
    Object.assign(ghost.style, {
      position: 'fixed',
      left: `${r.left}px`,
      top: `${r.top}px`,
      width: `${r.width}px`,
      height: `${r.height}px`,
      margin: '0',
      zIndex: '1000',
      pointerEvents: 'none',
      transition: reduced() ? 'none' : 'box-shadow 160ms ease, opacity 160ms ease',
    })
    document.body.appendChild(ghost)
    requestAnimationFrame(() => ghost?.classList.add('lifted'))
    st.order = [...opts.ids()]
    st.dragging = id
    document.body.classList.add('is-sorting')
  }

  function place(x: number, y: number) {
    if (!ghost) return
    ghost.style.transform = `translate(${x - sx}px, ${y - sy}px)`
  }

  /** The item under the pointer (or the nearest one) decides the new slot. */
  function retarget(x: number, y: number) {
    if (!st.order) return
    const els = [...document.querySelectorAll<HTMLElement>('[data-sort-id]')].filter((el) => !el.classList.contains('sort-ghost'))
    let best: { id: string; d: number } | null = null
    for (const el of els) {
      const r = el.getBoundingClientRect()
      const inside = x >= r.left && x <= r.right && y >= r.top && y <= r.bottom
      const d = inside ? -1 : Math.hypot(x - (r.left + r.width / 2), y - (r.top + r.height / 2))
      if (!best || d < best.d) best = { id: el.dataset.sortId!, d }
    }
    if (!best || best.id === id) return
    const to = st.order.indexOf(best.id)
    const from = st.order.indexOf(id)
    if (to < 0 || from < 0) return
    const next = [...st.order]
    next.splice(to, 0, next.splice(from, 1)[0])
    st.order = next
  }

  function autoscroll(y: number) {
    const el = opts.scroller?.()
    if (!el) return
    const r = el.getBoundingClientRect()
    const edge = 48
    const dy = y < r.top + edge ? -(r.top + edge - y) : y > r.bottom - edge ? y - (r.bottom - edge) : 0
    if (dy && !scrollTimer) {
      scrollTimer = window.setInterval(() => {
        el.scrollBy(0, Math.sign(dy) * 12)
        retarget(lastX, lastY)
      }, 16)
    } else if (!dy && scrollTimer) {
      clearInterval(scrollTimer)
      scrollTimer = 0
    }
  }

  function move(e: PointerEvent) {
    lastX = e.clientX
    lastY = e.clientY
    if (!started) {
      if (Math.hypot(e.clientX - sx, e.clientY - sy) < THRESHOLD) return
      begin()
    }
    place(e.clientX, e.clientY)
    retarget(e.clientX, e.clientY)
    autoscroll(e.clientY)
  }

  function swallowClick(ev: MouseEvent) {
    ev.stopPropagation()
    ev.preventDefault()
  }

  function finish(commit: boolean) {
    removeEventListener('pointermove', move)
    removeEventListener('pointerup', up)
    removeEventListener('pointercancel', cancel)
    removeEventListener('keydown', key)
    clearInterval(scrollTimer)
    scrollTimer = 0
    if (!started) {
      source = null
      return
    }
    addEventListener('click', swallowClick, { capture: true, once: true })
    setTimeout(() => removeEventListener('click', swallowClick, { capture: true }), 0)

    const original = opts.ids()
    const order = commit ? st.order : null
    if (!commit) st.order = [...original]
    const g = ghost
    // Let the list settle, then glide the ghost into the placeholder.
    requestAnimationFrame(() => {
      const slot = document.querySelector<HTMLElement>(`[data-sort-id="${CSS.escape(id)}"]:not(.sort-ghost)`)
      if (g && slot) {
        const r = slot.getBoundingClientRect()
        g.classList.remove('lifted')
        g.style.transition = reduced() ? 'none' : `transform ${GLIDE_MS}ms cubic-bezier(.2,.8,.2,1), box-shadow ${GLIDE_MS}ms ease`
        g.style.transform = `translate(${r.left - origin.left}px, ${r.top - origin.top}px)`
      }
      setTimeout(
        () => {
          g?.remove()
          st.dragging = null
          document.body.classList.remove('is-sorting')
          if (order && order.join() !== original.join()) opts.commit(order)
          // Keep showing the new order until the saved one arrives, so nothing jumps back.
          setTimeout(() => (st.order = null), order ? 600 : 0)
        },
        reduced() ? 0 : GLIDE_MS,
      )
    })
    ghost = null
    source = null
    started = false
  }

  function up() {
    finish(true)
  }
  function cancel() {
    finish(false)
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      finish(false)
    }
  }

  /** `items` rearranged to the order being previewed (or as given when not dragging). */
  function arrange<T>(items: T[], key: (t: T) => string): T[] {
    if (!st.order) return items
    const byId = new Map(items.map((t) => [key(t), t]))
    const out = st.order.map((i) => byId.get(i)).filter(Boolean) as T[]
    // Items that appeared meanwhile keep their place at the end.
    for (const t of items) if (!st.order.includes(key(t))) out.push(t)
    return out
  }

  return { st, down, arrange }
}

/** Puts a reordered visible subset back into the full order, keeping hidden items in place. */
export function mergeOrder(all: string[], visibleNew: string[]): string[] {
  const visible = new Set(visibleNew)
  let i = 0
  return all.map((id) => (visible.has(id) ? visibleNew[i++] : id))
}
