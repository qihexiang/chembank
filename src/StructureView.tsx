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
    setComponent,
    deleteComponent,
    removeStructure,
} from "./bindings";
import { Box, Button, Grid2, Slider, TextField, Typography } from "@mui/material";
import rdkitModule from "./rdkit";
import { message, open, confirm } from "@tauri-apps/api/dialog"
import mime from "mime";
import { readBinaryFile } from "@tauri-apps/api/fs";
import { basename } from "@tauri-apps/api/path";
import useFetch from "./useFetch";

type ViewState = {
    structure: Structure;
    components: [Component, Structure | null][];
    relateds: [Component, Structure | null][];
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
        await setImage(structure.id, image.image, image.filename)
    }
    await setProperty({ ...property, structure_id: structure.id })
}

const emptyProperty = {
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
};
export default function StructureView() {
    const navigate = useNavigate();
    const [searchParams] = useSearchParams();
    const currentId = searchParams.get("id");
    const componentOf = searchParams.get("component_of");
    const [state, setState] = useState<ViewState>({
        structure: {
            id: 0,
            formula: "",
            smiles: null,
            name: null,
            charge: 0,
        },
        components: [],
        relateds: [],
        property: emptyProperty,
        image: null,
    });

    const [componentCount, setComponentCount] = useState(1);

    const refresh = () => getStructureDetail(Number(currentId)).then(
        ([structure, property, image, components, relateds]) => {
            setState({ structure, property: property ?? { ...emptyProperty, structure_id: structure.id }, image, components, relateds });
        }
    );

    useEffect(() => {
        if (currentId !== null) {
            refresh()
        }
    }, [currentId]);

    useEffect(() => {
        return () => {
            getStructureDetail(Number(currentId)).then(([structure, ..._]) => {
                if (structure.name === null && structure.smiles === null) {
                    alert("请设置名称或SMILES")
                    if (componentOf !== null) {
                        navigate(`/structre?id=${currentId}&component_of=${componentOf}`)
                    } else {
                        navigate(`/structure?id=${currentId}`)
                    }
                }
            })
        }
    }, [])

    if (currentId === null) {
        return <Box>
            <Typography>似乎发生了一些问题</Typography>
            <Button variant="contained" color="primary" onClick={() => navigate("/")}>返回首页</Button>
        </Box>
    }

    return (
        <Box display={"flex"} flexDirection={"column"} gap={2}>
            <Box display={"flex"} gap={2}>
                <Typography variant="h5">详细信息</Typography>

                {componentOf === null ? <>
                    <Button variant="contained" color="success" onClick={() => updateToDB(state).then(refresh).catch(e => message(e))}>保存</Button>
                    <Button variant="contained" color="primary" onClick={() => updateToDB(state).then(() => navigate("/"))}>保存并返回首页</Button>
                    <Button variant="contained" color="error" onClick={() => removeStructure(Number(currentId)).then(() => navigate("/"))}>删除并返回首页</Button>
                </> :
                    <>
                        <TextField label="子结构数目" value={componentCount} onChange={(e) => setComponentCount(Number(e.target.value))}></TextField>
                        <Button variant="contained" color="success" onClick={async () => {
                            await updateToDB(state)
                            await setComponent(Number(componentOf), state.structure.id, componentCount)
                            navigate(`/structure?id=${componentOf}`)
                        }}>添加到子结构并返回</Button>
                        <Button variant="contained" color="warning" onClick={async () => {
                            navigate(`/structure?id=${componentOf}`)
                        }}>取消并返回</Button>
                    </>}
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
                <Box width={256} height={256}>{
                    state.image !== null ? <img style={{ maxWidth: "100%", maxHeight: "100%", objectFit: "contain" }} src={URL.createObjectURL(new Blob([Uint8Array.from(state.image.image)], { type: mime.getType(state.image.filename) ?? `image/png` }))}></img> : <Typography>图像未上传</Typography>
                }</Box>
                <Grid2 container alignItems={"center"} justifyContent={"space-between"} spacing={2}>
                    <Button variant={"contained"} onClick={async () => {
                        const filepath = await open({
                            filters: [{
                                name: 'Image',
                                extensions: ['png', 'jpeg', 'svg', 'bmp', 'webp', 'gif', 'apng', 'tiff', 'tif', 'heif', 'heic']
                            }]
                        });
                        if (filepath !== null) {
                            const fileContent = await readBinaryFile(filepath as string)
                            const filename = await basename(filepath as string);
                            setState({ ...state, image: { structure_id: state.structure.id, filename, image: [...fileContent] } })
                        }
                    }}>选择图片</Button>
                    {state.structure.smiles !== null ? <Button variant={"contained"} color="secondary" onClick={async () => {
                        const svg = await smilesToSVG(state.structure.smiles!);
                        if (svg !== undefined) {
                            const encoder = new TextEncoder();
                            const encode = encoder.encode(svg);
                            const image = [...encode];
                            setState({ ...state, image: { structure_id: state.structure.id, image, filename: "rdkit.svg" } })
                        } else {
                            await message("给出的SMILES似乎不正确")
                        }
                    }}>根据SMILES生成</Button> : null}
                </Grid2>
            </Box>
            <Box>
                <Grid2 container alignItems={"center"} justifyContent={"center"} spacing={2}>
                    <TextField label="分解温度（℃）" placeholder="热分解温度" value={state.property.decomp_temp ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, decomp_temp: e.target.value } })}></TextField>
                    <TextField label="热熔解温度（℃）" placeholder="热溶解温度" value={state.property.diss_temp ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, diss_temp: e.target.value } })}></TextField>
                    <TextField label="密度（g·cm-3）" placeholder="密度" value={state.property.density ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, density: e.target.value } })}></TextField>
                    <TextField label="生成焓（kJ·mol-1）" placeholder="生成焓" value={state.property.formation_enthalpy ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, formation_enthalpy: e.target.value } })}></TextField>
                    <TextField label="撞击感度（J）" placeholder="撞击感度" value={state.property.impact_sensitive ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, impact_sensitive: e.target.value } })}></TextField>
                    <TextField label="摩擦感度（N）" placeholder="摩擦感度" value={state.property.friction_sensitivity ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, friction_sensitivity: e.target.value } })}></TextField>
                    <TextField label="爆速（ms-1）" placeholder="爆速" value={state.property.det_velocity ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, det_velocity: e.target.value } })}></TextField>
                    <TextField label="爆压（GPa）" placeholder="爆压" value={state.property.det_pressure ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, det_pressure: e.target.value } })}></TextField>
                    <TextField label="氮含量（%）" placeholder="氮含量" value={state.property.n_content ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, n_content: e.target.value } })}></TextField>
                    <TextField label="氧含量（%）" placeholder="氧含量" value={state.property.o_content ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, o_content: e.target.value } })}></TextField>
                    <TextField label="氮氧含量（%）" placeholder="氮氧含量" value={state.property.no_content ?? 0.} onChange={(e) => setState({ ...state, property: { ...state.property, no_content: e.target.value } })}></TextField>
                    <Grid2 size={12}>
                        <TextField fullWidth multiline label="参考文献" placeholder="请填写DOI号（可填写多行内容）" value={state.property.references ?? ""} onChange={(e) => setState({ ...state, property: { ...state.property, references: e.target.value } })}></TextField>
                    </Grid2>
                    <Grid2 size={12}>
                        <TextField fullWidth multiline label="备注" placeholder="备注（可填写多行内容）" value={state.property.remarks ?? ""} onChange={(e) => setState({ ...state, property: { ...state.property, remarks: e.target.value } })}></TextField>
                    </Grid2>
                </Grid2>
            </Box>
            <Box display={"flex"} flexDirection={"column"} gap={2}>
                <Typography variant="h6">子结构</Typography>
                <Grid2 container spacing={2}>
                    <Box display={"flex"} justifyContent={"center"} alignItems={"stretch"} flexDirection={"row"} gap={2} flexWrap={"wrap"}>
                        {
                            state.components.map(([component, structure], index) => <ComponentItem ro={false} key={index} component={component} structure={structure!} callback={refresh}></ComponentItem>)
                        }
                        <Button variant="contained" onClick={async () => {
                            const answer = await confirm("添加子结构前，是否要保存已经填写的信息？")
                            if (answer) {
                                await updateToDB(state)
                            }
                            navigate(`/component?component_of=${state.structure.id}`)
                        }}>添加子结构</Button>
                    </Box>
                </Grid2>
            </Box>
            <Box display={"flex"} flexDirection={"column"} gap={2}>
                <Typography variant="h6">相关结构</Typography>
                <Grid2 container spacing={2}>
                    <Box display={"flex"} justifyContent={"center"} alignItems={"stretch"} flexDirection={"row"} gap={2} flexWrap={"wrap"}>
                        {
                            state.relateds.map(([component, structure], index) => <ComponentItem key={index} component={component} structure={structure!} callback={refresh} ro></ComponentItem>)
                        }
                    </Box>
                </Grid2>
            </Box>
        </Box>
    );
}

function ComponentItem(props: { component: Component, structure: Structure, callback: () => void, ro: boolean }) {
    const navigate = useNavigate();
    const [component, updateComponent] = useState(props.component)
    const { structure } = props;
    const [detail] = useFetch(() => getStructureDetail(structure.id), [structure, null, null, [], []], [structure.id])
    const image = detail[2];
    useEffect(() => {
        setComponent(component.structure_id, component.component_id, component.count).then(props.callback)
    }, [component])
    return <Box gap={1} width={256} display={"flex"} flexDirection={"column"} alignItems={"stretch"} justifyContent={"stretch"}>
        <Box height={256} display={"flex"} alignItems={"center"} justifyContent={"center"}>{
            image !== null ? <img style={{ maxWidth: "100%", maxHeight: "100%", objectFit: "contain" }} src={URL.createObjectURL(new Blob([Uint8Array.from(image.image)], { type: mime.getType(image.filename) ?? `image/png` }))}></img> : <Typography>图像未上传</Typography>
        }</Box>
        {structure.name !== null ? <Typography>名称：{structure.name}</Typography> : null}
        <Typography>分子式：{structure.formula}</Typography>
        {structure.smiles !== null ? <Typography>SMILES：{structure.smiles}</Typography> : null}
        {structure.charge !== null ? <Typography>电荷：{structure.charge}</Typography> : null}
        <TextField fullWidth label="数量" value={component.count} onChange={async (e) => {
            updateComponent({ ...component, count: Number(e.target.value) })
        }}></TextField>
        {props.ro ? null : <Button variant="contained" color="error" onClick={() => deleteComponent(component.structure_id, component.component_id).then(props.callback)}>删除</Button>}
        <Button variant="contained" color="info" onClick={() => navigate(`/structure?id=${structure.id}`)}>查看</Button>
    </Box>
}
