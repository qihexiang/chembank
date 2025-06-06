/* eslint-disable */
// This file was generated by [tauri-specta](https://github.com/oscartbeaumont/tauri-specta). Do not edit this file manually.

declare global {
    interface Window {
        __TAURI_INVOKE__<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
    }
}

// Function avoids 'window not defined' in SSR
const invoke = () => window.__TAURI_INVOKE__;

export function resetDatabase() {
    return invoke()<null>("reset_database")
}

export function createStructure(name: string | null, formula: string, smiles: string | null, charge: number) {
    return invoke()<number>("create_structure", { name,formula,smiles,charge })
}

export function updateStructure(id: number, name: string | null, formula: string, smiles: string | null, charge: number) {
    return invoke()<null>("update_structure", { id,name,formula,smiles,charge })
}

export function removeStructure(id: number) {
    return invoke()<null>("remove_structure", { id })
}

export function setComponent(structureId: number, componentId: number, count: number) {
    return invoke()<null>("set_component", { structureId,componentId,count })
}

export function deleteComponent(structureId: number, componentId: number) {
    return invoke()<null>("delete_component", { structureId,componentId })
}

export function setImage(structureId: number, image: number[], filename: string) {
    return invoke()<null>("set_image", { structureId,image,filename })
}

export function setProperty(model: Property) {
    return invoke()<null>("set_property", { model })
}

export function searchStructure(pageSize: number, pageNumber: number, keyword: string | null, maxCharge: number, minCharge: number) {
    return invoke()<[Structure[], number]>("search_structure", { pageSize,pageNumber,keyword,maxCharge,minCharge })
}

export function getStructureDetail(id: number) {
    return invoke()<[Structure, Property | null, Image | null, ([Component, Structure | null])[], ([Component, Structure | null])[]]>("get_structure_detail", { id })
}

export function exportToFolder(folderPath: string) {
    return invoke()<null>("export_to_folder", { folderPath })
}

export function importFromFolder(folderPath: string) {
    return invoke()<null>("import_from_folder", { folderPath })
}

export type Component = { structure_id: number; component_id: number; count: number }
export type Structure = { id: number; name: string | null; formula: string; smiles: string | null; charge: number }
export type Property = { structure_id: number; decomp_temp: number | null; density: number | null; diss_temp: number | null; formation_enthalpy: number | null; impact_sensitive: number | null; friction_sensitivity: number | null; det_velocity: number | null; det_pressure: number | null; n_content: number | null; o_content: number | null; no_content: number | null; references: string | null; remarks: string | null }
export type Image = { structure_id: number; filename: string; image: number[] }
