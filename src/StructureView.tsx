import { useEffect, useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import {
    getStructureDetail,
    Image,
    Property,
    Structure,
    Component,
    updateStructure,
    setImage,
    setProperty,
} from "./bindings";
import { Box, Button, Grid2, Slider, TextField, Typography } from "@mui/material";
import rdkitModule from "./rdkit";
import { message } from "@tauri-apps/api/dialog"

type ViewState = {
    structure: Structure;
    components: [Component, Structure | null][];
    property: Property;
    image: Image | null;
};

async function smilesToSVG(smiles: string) {
    const rdkit = await rdkitModule;
    const mol = rdkit.get_mol(smiles);
    const svg = mol?.get_svg();
    return svg
}

async function smilesToCanonical(smiles: string) {
    const rdkit = await rdkitModule;
    const mol = rdkit.get_mol(smiles);
    return mol?.get_smiles() ?? null
}

async function updateToDB(state: ViewState) {
    const { structure, image, property } = state;
    await updateStructure(structure.id, structure.name, structure.formula, structure.smiles && await smilesToCanonical(structure.smiles), structure.charge);
    if (image !== null) {
        await setImage(structure.id, image.image, image.extension)
    }
    await setProperty({ ...property, structure_id: structure.id })
}

export default function StructureView() {
    const navigate = useNavigate();
    const [searchParams] = useSearchParams();
    const current_id = searchParams.get("id");
    const [state, setState] = useState<ViewState>({
        structure: {
            id: 0,
            formula: "",
            smiles: null,
            name: null,
            charge: 0,
        },
        components: [],
        property: {
            structure_id: 0,
            decomp_temp: null,
            density: null,
            diss_temp: null,
            formation_enthalpy: null,
            impact_sensitive: null,
            friction_sensitivity: null,
            det_velocity: null,
            det_pressure: null,
            n_content: null,
            o_content: null,
            no_content: null,
            references: "",
            remarks: ""
        },
        image: null,
    });

    const refresh = () => getStructureDetail(Number(current_id)).then(
        ([structure, property, image, components]) => {
            setState({ structure, property: property ?? { ...state.property, structure_id: structure.id }, image, components });
        }
    );

    useEffect(() => {
        if (current_id !== null) {
            refresh()
        }
    }, [current_id]);

    if (current_id === null) {
        return <Box>
            <Typography>似乎发生了一些问题</Typography>
            <Button variant="contained" color="primary" onClick={() => navigate("/")}>返回首页</Button>
        </Box>
    }

    return (
        <Box display={"flex"} flexDirection={"column"} gap={2}>
            <Box display={"flex"} gap={2}>
                <Typography variant="h5">详细信息</Typography>
                <Button variant="contained" color="success" onClick={() => updateToDB(state).then(refresh)}>保存</Button>
                <Button variant="contained" color="primary" onClick={() => navigate("/")}>返回首页</Button>
            </Box>
            <Box display={"flex"} flexDirection={"column"} gap={2}>
                <Grid2 container alignItems={"center"} justifyContent={"space-between"} spacing={2}>
                    <TextField label="ID" value={`${state.structure.id}`} disabled></TextField>
                    <TextField label="名称" placeholder="输入名称（可选）" value={state.structure.name ?? ""} onChange={(e) => setState({ ...state, structure: { ...state.structure, name: e.target.value === "" ? null : e.target.value } })}></TextField>
                    <TextField label="分子式" placeholder="输入分子式" value={state.structure.formula} onChange={(e) => setState({ ...state, structure: { ...state.structure, formula: e.target.value } })}></TextField>
                    <TextField label="SMILES（将以标准形式存储）" placeholder="输入SMILES（可选）" value={state.structure.smiles ?? ""} onChange={(e) => setState({ ...state, structure: { ...state.structure, smiles: e.target.value === "" ? null : e.target.value } })}></TextField>
                    <Box sx={{ width: 192 }}>
                        <Typography id="charge_label">电荷</Typography>
                        <Slider
                            aria-labelledby="charge_label"
                            marks={new Array(21)
                                .map((_, index) => index - 10)
                                .map((value) => ({ value, label: String(value) }))}
                            valueLabelDisplay="on"
                            min={-10}
                            max={10}
                            step={1}
                            value={state.structure.charge}
                            onChange={(_, v) =>
                                setState({
                                    ...state,
                                    structure: { ...state.structure, charge: v as number },
                                })
                            }
                        ></Slider>
                    </Box>
                </Grid2>
            </Box>
            <Box display={"flex"} flexDirection={"row"} gap={2}>
                <Box width={128} height={128}>{
                    state.image !== null ? <img style={{ maxWidth: "100%", maxHeight: "100%", objectFit: "contain" }} src={URL.createObjectURL(new Blob([Uint8Array.from(state.image.image)], { type: `image/${state.image.extension}` }))}></img> : <Typography>图像未上传</Typography>
                }</Box>
                <Grid2 container alignItems={"center"} justifyContent={"space-between"} spacing={2}>
                    <Button variant={"contained"} onClick={async () => {

                    }}>选择图片</Button>
                    {state.structure.smiles !== null ? <Button variant={"contained"} color="secondary" onClick={async () => {
                        const svg = await smilesToSVG(state.structure.smiles!);
                        if (svg !== undefined) {
                            const encoder = new TextEncoder();
                            const encode = encoder.encode(svg);
                            const image = [...encode];
                            setState({ ...state, image: { structure_id: state.structure.id, image, extension: "svg+xml" } })
                        } else {
                            await message("给出的SMILES似乎不正确")
                        }
                    }}>根据SMILES生成</Button> : null}
                </Grid2>
            </Box>
            <Box>
                <Grid2 container alignItems={"center"} justifyContent={"center"} spacing={2}>
                    <TextField type="number" label="分解温度（℃）" placeholder="热分解温度" value={state.property.decomp_temp ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, decomp_temp: Number(e.target.value) } })}></TextField>
                    <TextField type="number" label="热熔解温度（℃）" placeholder="热溶解温度" value={state.property.diss_temp ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, diss_temp: Number(e.target.value) } })}></TextField>
                    <TextField type="number" label="密度（g·cm-3）" placeholder="密度" value={state.property.density ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, density: Number(e.target.value) } })}></TextField>
                    <TextField type="number" label="生成焓（kJ·mol-1）" placeholder="生成焓" value={state.property.formation_enthalpy ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, formation_enthalpy: Number(e.target.value) } })}></TextField>
                    <TextField type="number" label="撞击感度（J）" placeholder="撞击感度" value={state.property.impact_sensitive ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, impact_sensitive: Number(e.target.value) } })}></TextField>
                    <TextField type="number" label="摩擦感度（N）" placeholder="摩擦感度" value={state.property.friction_sensitivity ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, friction_sensitivity: Number(e.target.value) } })}></TextField>
                    <TextField type="number" label="爆速（ms-1）" placeholder="爆速" value={state.property.det_velocity ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, det_velocity: Number(e.target.value) } })}></TextField>
                    <TextField type="number" label="爆压（GPa）" placeholder="爆压" value={state.property.det_pressure ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, det_pressure: Number(e.target.value) } })}></TextField>
                    <TextField type="number" label="氮含量（%）" placeholder="氮含量" value={state.property.n_content ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, n_content: Number(e.target.value) } })}></TextField>
                    <TextField type="number" label="氧含量（%）" placeholder="氧含量" value={state.property.o_content ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, o_content: Number(e.target.value) } })}></TextField>
                    <TextField type="number" label="氮氧含量（%）" placeholder="氮氧含量" value={state.property.no_content ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, no_content: Number(e.target.value) } })}></TextField>
                    <Grid2 size={12}>
                        <TextField fullWidth multiline label="参考文献" placeholder="请填写DOI号（可填写多行内容）" value={state.property.references ?? ""} onChange={(e) => setState({ ...state, property: { ...state.property, references: e.target.value } })}></TextField>
                    </Grid2>
                    <Grid2 size={12}>
                        <TextField fullWidth multiline label="备注" placeholder="备注（可填写多行内容）" value={state.property.remarks ?? ""} onChange={(e) => setState({ ...state, property: { ...state.property, remarks: e.target.value } })}></TextField>
                    </Grid2>
                </Grid2>
            </Box>
        </Box>
    );
}
