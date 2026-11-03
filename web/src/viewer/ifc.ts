// SPDX-License-Identifier: AGPL-3.0-or-later
//! IFC -> three.js through web-ifc, one mesh per element, tagged with its GlobalId.
import * as THREE from 'three'
import * as WebIFC from 'web-ifc'

export interface ElementInfo {
  expressId: number
  guid: string
  type: string
  name: string
}

export interface LoadedModel {
  group: THREE.Group
  elements: Map<string, { mesh: THREE.Mesh; info: ElementInfo }>
  /** Read the element's attributes and property sets from the model (lazy, on selection). */
  properties(guid: string): Promise<PropertyGroup[]>
  dispose(): void
}

export interface PropertyGroup {
  name: string
  values: [string, string][]
}

let apiPromise: Promise<WebIFC.IfcAPI> | null = null

function ifcApi(): Promise<WebIFC.IfcAPI> {
  if (!apiPromise) {
    apiPromise = (async () => {
      const api = new WebIFC.IfcAPI()
      api.SetWasmPath('/wasm/', true)
      await api.Init()
      return api
    })()
  }
  return apiPromise
}

const materials = new Map<string, THREE.MeshLambertMaterial>()

export function material(color: THREE.Color, opacity: number): THREE.MeshLambertMaterial {
  const key = `${color.getHexString()}:${opacity.toFixed(2)}`
  let m = materials.get(key)
  if (!m) {
    m = new THREE.MeshLambertMaterial({
      color,
      transparent: opacity < 1,
      opacity,
      side: THREE.DoubleSide,
      depthWrite: opacity >= 1,
      polygonOffset: true,
      polygonOffsetFactor: 1,
      polygonOffsetUnits: 1,
    })
    materials.set(key, m)
  }
  return m
}

function text(v: unknown): string {
  if (v === null || v === undefined) return ''
  if (typeof v === 'object' && v !== null && 'value' in v) return text((v as { value: unknown }).value)
  if (Array.isArray(v)) return v.map(text).join(', ')
  return String(v)
}

/** Parse an IFC file into meshes. `only` keeps just those GlobalIds (used for ghosts of removed elements). */
export async function loadIfc(data: Uint8Array, only?: Set<string>): Promise<LoadedModel> {
  const api = await ifcApi()
  const modelId = api.OpenModel(data, { COORDINATE_TO_ORIGIN: true })
  const group = new THREE.Group()
  const elements = new Map<string, { mesh: THREE.Mesh; info: ElementInfo }>()
  const byExpress = new Map<number, { positions: number[]; normals: number[]; indices: number[]; color: THREE.Color; opacity: number }>()

  api.StreamAllMeshes(modelId, (flat) => {
    const id = flat.expressID
    const placed = flat.geometries
    for (let i = 0; i < placed.size(); i++) {
      const pg = placed.get(i)
      const geom = api.GetGeometry(modelId, pg.geometryExpressID)
      const verts = api.GetVertexArray(geom.GetVertexData(), geom.GetVertexDataSize())
      const idx = api.GetIndexArray(geom.GetIndexData(), geom.GetIndexDataSize())
      const m = new THREE.Matrix4().fromArray(pg.flatTransformation)
      const nm = new THREE.Matrix3().getNormalMatrix(m)
      let entry = byExpress.get(id)
      if (!entry) {
        entry = { positions: [], normals: [], indices: [], color: new THREE.Color(pg.color.x, pg.color.y, pg.color.z), opacity: pg.color.w }
        byExpress.set(id, entry)
      }
      const base = entry.positions.length / 3
      const p = new THREE.Vector3()
      const n = new THREE.Vector3()
      for (let v = 0; v < verts.length; v += 6) {
        p.set(verts[v], verts[v + 1], verts[v + 2]).applyMatrix4(m)
        n.set(verts[v + 3], verts[v + 4], verts[v + 5]).applyMatrix3(nm).normalize()
        entry.positions.push(p.x, p.y, p.z)
        entry.normals.push(n.x, n.y, n.z)
      }
      for (let k = 0; k < idx.length; k++) entry.indices.push(base + idx[k])
      geom.delete()
    }
  })

  for (const [id, e] of byExpress) {
    let line: Record<string, unknown>
    try {
      line = api.GetLine(modelId, id) as Record<string, unknown>
    } catch {
      continue
    }
    const guid = text(line.GlobalId)
    if (!guid || (only && !only.has(guid))) continue
    const g = new THREE.BufferGeometry()
    g.setAttribute('position', new THREE.Float32BufferAttribute(e.positions, 3))
    g.setAttribute('normal', new THREE.Float32BufferAttribute(e.normals, 3))
    g.setIndex(e.indices)
    const mesh = new THREE.Mesh(g, material(e.color, e.opacity))
    const info: ElementInfo = { expressId: id, guid, type: api.GetNameFromTypeCode(api.GetLineType(modelId, id)), name: text(line.Name) }
    mesh.userData = { guid, baseColor: e.color, baseOpacity: e.opacity }
    group.add(mesh)
    elements.set(guid, { mesh, info })
  }

  async function properties(guid: string): Promise<PropertyGroup[]> {
    const el = elements.get(guid)
    if (!el) return []
    const line = api.GetLine(modelId, el.info.expressId, false) as Record<string, unknown>
    const attrs: [string, string][] = []
    for (const k of ['GlobalId', 'Name', 'Description', 'ObjectType', 'Tag', 'PredefinedType', 'LongName']) {
      const v = text(line[k])
      if (v) attrs.push([k, v])
    }
    const out: PropertyGroup[] = [{ name: el.info.type, values: attrs }]
    try {
      const psets = (await api.properties.getPropertySets(modelId, el.info.expressId, true)) as Record<string, unknown>[]
      for (const ps of psets) {
        const values: [string, string][] = []
        for (const prop of (ps.HasProperties as Record<string, unknown>[] | undefined) ?? []) {
          values.push([text(prop.Name), text(prop.NominalValue ?? prop.EnumerationValues ?? '')])
        }
        for (const q of (ps.Quantities as Record<string, unknown>[] | undefined) ?? []) {
          const val = Object.entries(q).find(([k]) => k.endsWith('Value'))?.[1]
          values.push([text(q.Name), text(val)])
        }
        if (values.length) out.push({ name: text(ps.Name), values })
      }
    } catch {
      // Property sets are optional; the element's attributes still show.
    }
    return out
  }

  return {
    group,
    elements,
    properties,
    dispose() {
      group.traverse((o) => {
        if (o instanceof THREE.Mesh) o.geometry.dispose()
      })
      try {
        api.CloseModel(modelId)
      } catch {
        // already closed
      }
    },
  }
}
