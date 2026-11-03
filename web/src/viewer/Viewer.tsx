// SPDX-License-Identifier: AGPL-3.0-or-later
import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { loadIfc, material, type LoadedModel, type PropertyGroup } from './ifc'
import { CHANGE_COLORS, type ChangeKind } from './colors'
import { useT } from '../i18n'
import { fetchModel } from '../lib/api'

export interface ViewerProps {
  /** API path of the version's IFC (`…/versions/{rev}/model.ifc`). */
  model: string
  /** GlobalId -> change kind: coloured; everything else is drawn pale. */
  highlight?: Record<string, ChangeKind>
  /** API path of the earlier version, to draw removed elements as red ghosts. */
  ghostModel?: string
  selected?: string | null
  onSelect?: (guid: string | null, props: PropertyGroup[]) => void
  children?: React.ReactNode
}

const PALE = new THREE.Color('#dfe4e7')

export function Viewer({ model, highlight, ghostModel, selected, onSelect, children }: ViewerProps) {
  const t = useT()
  const host = useRef<HTMLDivElement>(null)
  const [status, setStatus] = useState<{ kind: 'loading' | 'exporting' | 'ready' | 'failed'; progress?: number; msg?: string }>({ kind: 'loading' })
  const scene = useRef<{ renderer: THREE.WebGLRenderer; camera: THREE.PerspectiveCamera; controls: OrbitControls; root: THREE.Scene; render: () => void } | null>(null)
  const loaded = useRef<LoadedModel | null>(null)
  const ghost = useRef<LoadedModel | null>(null)
  const selRef = useRef<string | null>(null)
  const onSelectRef = useRef(onSelect)
  onSelectRef.current = onSelect

  // Scene, once.
  useEffect(() => {
    const el = host.current!
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true })
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2))
    el.appendChild(renderer.domElement)
    const camera = new THREE.PerspectiveCamera(45, 1, 0.05, 20000)
    camera.position.set(30, 25, 30)
    const controls = new OrbitControls(camera, renderer.domElement)
    controls.enableDamping = false
    const root = new THREE.Scene()
    root.add(new THREE.HemisphereLight(0xffffff, 0x8a96a0, 2.2))
    const sun = new THREE.DirectionalLight(0xffffff, 1.4)
    sun.position.set(40, 80, 20)
    root.add(sun)
    const render = () => renderer.render(root, camera)
    controls.addEventListener('change', render)
    const resize = () => {
      const { clientWidth: w, clientHeight: h } = el
      if (!w || !h) return
      renderer.setSize(w, h)
      camera.aspect = w / h
      camera.updateProjectionMatrix()
      render()
    }
    const ro = new ResizeObserver(resize)
    ro.observe(el)
    resize()
    // Click (not drag) selects.
    let down: { x: number; y: number } | null = null
    const onDown = (e: PointerEvent) => (down = { x: e.clientX, y: e.clientY })
    const onUp = (e: PointerEvent) => {
      if (!down || Math.hypot(e.clientX - down.x, e.clientY - down.y) > 4 || !loaded.current) return
      const rect = renderer.domElement.getBoundingClientRect()
      const ndc = new THREE.Vector2(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1)
      const ray = new THREE.Raycaster()
      ray.setFromCamera(ndc, camera)
      const hit = ray.intersectObjects(loaded.current.group.children, false).find((h) => (h.object as THREE.Mesh).visible)
      const guid = (hit?.object.userData.guid as string | undefined) ?? null
      select(guid)
    }
    renderer.domElement.addEventListener('pointerdown', onDown)
    renderer.domElement.addEventListener('pointerup', onUp)
    scene.current = { renderer, camera, controls, root, render }
    return () => {
      ro.disconnect()
      renderer.domElement.removeEventListener('pointerdown', onDown)
      renderer.domElement.removeEventListener('pointerup', onUp)
      controls.dispose()
      renderer.dispose()
      el.removeChild(renderer.domElement)
      scene.current = null
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  function fit() {
    const s = scene.current
    if (!s || !loaded.current) return
    const box = new THREE.Box3().setFromObject(loaded.current.group)
    if (box.isEmpty()) return
    const size = box.getSize(new THREE.Vector3()).length()
    const center = box.getCenter(new THREE.Vector3())
    s.controls.target.copy(center)
    s.camera.position.copy(center).add(new THREE.Vector3(0.75, 0.55, 0.9).multiplyScalar(size * 0.85))
    s.camera.near = size / 1000
    s.camera.far = size * 20
    s.camera.updateProjectionMatrix()
    s.controls.update()
    s.render()
  }

  function paint() {
    const m = loaded.current
    if (!m) return
    for (const [guid, { mesh }] of m.elements) {
      const kind = highlight?.[guid]
      const base = mesh.userData.baseColor as THREE.Color
      const opacity = mesh.userData.baseOpacity as number
      let color = base
      let alpha = opacity
      if (highlight) {
        color = kind ? new THREE.Color(CHANGE_COLORS[kind]) : PALE
        alpha = kind ? Math.max(opacity, 0.95) : Math.min(opacity, 0.35)
      }
      if (guid === selRef.current) color = new THREE.Color('#24508F')
      mesh.material = material(color, alpha)
    }
    scene.current?.render()
  }

  async function select(guid: string | null) {
    selRef.current = guid
    paint()
    const props = guid && loaded.current ? await loaded.current.properties(guid) : []
    onSelectRef.current?.(guid, props)
  }

  // Load the model whenever it changes.
  useEffect(() => {
    let cancelled = false
    setStatus({ kind: 'loading' })
    ;(async () => {
      try {
        const data = await fetchModel(model, (p) => !cancelled && setStatus({ kind: 'exporting', progress: p }))
        if (cancelled) return
        setStatus({ kind: 'loading' })
        const m = await loadIfc(data)
        if (cancelled) return m.dispose()
        loaded.current?.dispose()
        if (loaded.current) scene.current?.root.remove(loaded.current.group)
        loaded.current = m
        scene.current?.root.add(m.group)
        paint()
        fit()
        setStatus({ kind: 'ready' })
      } catch (e) {
        if (!cancelled) setStatus({ kind: 'failed', msg: (e as Error).message })
      }
    })()
    return () => {
      cancelled = true
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [model])

  // Ghosts of removed elements from the earlier version.
  useEffect(() => {
    const removed = new Set(Object.entries(highlight ?? {}).filter(([, k]) => k === 'removed').map(([g]) => g))
    if (!ghostModel || removed.size === 0) return
    let cancelled = false
    ;(async () => {
      try {
        const data = await fetchModel(ghostModel, () => {})
        const g = await loadIfc(data, removed)
        if (cancelled) return g.dispose()
        for (const { mesh } of g.elements.values()) mesh.material = material(new THREE.Color(CHANGE_COLORS.removed), 0.45)
        ghost.current = g
        scene.current?.root.add(g.group)
        scene.current?.render()
      } catch {
        // Ghosts are a convenience; the change list still names removed elements.
      }
    })()
    return () => {
      cancelled = true
      if (ghost.current) {
        scene.current?.root.remove(ghost.current.group)
        ghost.current.dispose()
        ghost.current = null
      }
    }
  }, [ghostModel, highlight])

  useEffect(paint, [highlight]) // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (selected === undefined || selected === selRef.current) return
    select(selected)
    // Frame the element.
    const el = selected ? loaded.current?.elements.get(selected) : null
    const s = scene.current
    if (el && s) {
      const box = new THREE.Box3().setFromObject(el.mesh)
      const c = box.getCenter(new THREE.Vector3())
      const size = Math.max(box.getSize(new THREE.Vector3()).length(), 2)
      const dir = s.camera.position.clone().sub(s.controls.target).normalize()
      s.controls.target.copy(c)
      s.camera.position.copy(c).add(dir.multiplyScalar(size * 3))
      s.controls.update()
      s.render()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected])

  useEffect(() => () => loaded.current?.dispose(), [])

  return (
    <div className="viewer">
      <div className="viewer-canvas" ref={host} />
      {status.kind !== 'ready' && (
        <div className="viewer-status" role="status">
          {status.kind === 'loading' && t('viewer.loading')}
          {status.kind === 'exporting' && t('viewer.exporting', { p: Math.round((status.progress ?? 0) * 100) })}
          {status.kind === 'failed' && t('viewer.failed', { msg: status.msg ?? '' })}
        </div>
      )}
      <div className="viewer-tools">
        <button type="button" className="btn-quiet" onClick={fit}>
          {t('viewer.fit')}
        </button>
      </div>
      {children}
    </div>
  )
}
