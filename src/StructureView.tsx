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
    generateStructureFromSmiles,
} from "./bindings";
import { Box, Button, Grid2, Slider, TextField, Typography } from "@mui/material";
import rdkitModule from "./rdkit";
import { message, open, confirm } from "@tauri-apps/api/dialog"
import mime from "mime";
import { readBinaryFile } from "@tauri-apps/api/fs";
import { basename } from "@tauri-apps/api/path";
import useFetch from "./useFetch";
import { calculateNMQ, calculateDP, analyseMoleculeFormula, explosionSimulate } from "./utils"

type ViewState = {
    structure: Structure;
    components: [Component, Structure | null][];
    relateds: [Component, Structure | null][];
    property: Property;
    image: Image | null;
};

async function smilesToCanonical(smiles: string) {
    const rdkit = await rdkitModule;
    const mol = rdkit.get_mol(smiles);
    return mol?.get_smiles() ?? null
}

// 阴、阳离子按电荷数配平的最小整数数目，例如 Na+ 与 CO3^2- 得到 2 与 1
function balancedIonCounts(cationCharge: number, anionCharge: number) {
    let divisor = cationCharge
    let remainder = -anionCharge
    while (remainder !== 0) {
        [divisor, remainder] = [remainder, divisor % remainder]
    }
    return { cation: -anionCharge / divisor, anion: cationCharge / divisor }
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

    // 子结构或相关结构变化后只刷新这两份列表，避免覆盖尚未保存的字段编辑
    const reloadLinks = () => getStructureDetail(Number(currentId)).then(
        ([, , , components, relateds]) => {
            setState(current => ({ ...current, components, relateds }))
        }
    );

    // 由 SMILES 生成各项信息，并同步子结构列表
    const generateFromSmiles = async (smiles: string) => {
        const info = await generateStructureFromSmiles(state.structure.id, smiles);
        setState(current => ({
            ...current,
            structure: {
                ...current.structure,
                formula: info.formula,
                charge: info.formal_charge,
                smiles,
            },
            property: {
                ...current.property,
                // 质量分数（0–1）转为属性栏使用的百分含量
                n_content: (info.n_mass_fraction * 100).toFixed(2),
                o_content: (info.o_mass_fraction * 100).toFixed(2),
            },
            image: {
                structure_id: current.structure.id,
                image: [...new TextEncoder().encode(info.svg)],
                filename: "rdkit.svg",
            },
        }))
        await reloadLinks()
    }

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

    // 仅当子结构恰好含一种阴离子和一种阳离子、且都存有 SMILES 时才能自动配平
    const ions = state.components.map(([, structure]) => structure).filter((structure): structure is Structure => structure !== null)
    const anions = ions.filter(ion => ion.charge < 0)
    const cations = ions.filter(ion => ion.charge > 0)
    const balanceable = ions.length === state.components.length && anions.length === 1 && cations.length === 1 && ions.every(ion => ion.smiles !== null)

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
                        try {
                            await generateFromSmiles(state.structure.smiles!)
                        } catch (e) {
                            await message(String(e))
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
                    <TextField label="氮氧含量（%）" placeholder="氮氧含量" value={(Number(state.property.n_content) ?? 0.) + (Number(state.property.o_content) ?? 0)} onChange={(e) => setState({ ...state, property: { ...state.property, no_content: e.target.value } })}></TextField>
                    <Button
                        variant="contained"
                        onClick={async () => {
                            if (state.property.formation_enthalpy === null) {
                                alert("生成焓未设置")
                                return 0
                            }
                            if (state.property.density === null) {
                                alert("密度未设置")
                                return 0
                            }
                            const atoms = analyseMoleculeFormula(state.structure.formula)
                            const gas = explosionSimulate(atoms)
                            const [N, M, Q] = calculateNMQ(atoms, gas, Number(state.property.formation_enthalpy))
                            if ([N,M,Q].map(value => value < 0).reduce((c,n) => c || n, false)) {
                                alert(`计算得到的N=${N.toFixed(4)}mol/g，M=${M.toFixed(4)}g/mol，Q=${Q.toFixed(4)}cal/g，包含负值，无法直接导出爆压爆速`)
                                return 0
                            }
                            const [D, P] = calculateDP(N, M, Q, Number(state.property.density))
                            if (await confirm(`生成气体：\n${Object.entries(gas).map(([gas, mol]) => `${gas}: ${mol} mol`).join("\n")}\n物理参数：\nN=${N.toFixed(4)}mol/g\nM=${M.toFixed(4)}g/mol\nQ=${Q.toFixed(4)}cal/g\n计算的爆炸热为${D.toFixed(4)}m/s，爆压为${P.toFixed(4)}GPa，要填入吗？`)) {
                                setState({ ...state, property: { ...state.property, det_pressure: String(P), det_velocity: String(D) } })
                            }
                        }}>
                        K-J方程估计爆压爆速（需要正确的分子式和反应物生成焓（单位kJ/mol））
                    </Button>
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
                            state.components.map(([component, structure], index) => <ComponentItem ro={false} key={index} component={component} structure={structure!} callback={reloadLinks}></ComponentItem>)
                        }
                        <Button variant="contained" onClick={async () => {
                            const answer = await confirm("添加子结构前，是否要保存已经填写的信息？")
                            if (answer) {
                                await updateToDB(state)
                            }
                            navigate(`/component?component_of=${state.structure.id}`)
                        }}>添加子结构</Button>
                        <Button variant="contained" disabled={!balanceable} onClick={async () => {
                            try {
                                const { cation: cationCount, anion: anionCount } = balancedIonCounts(cations[0].charge, anions[0].charge)
                                const smiles = state.components
                                    .map(([component, structure]) => {
                                        const count = structure!.id === cations[0].id ? cationCount : structure!.id === anions[0].id ? anionCount : component.count
                                        return new Array(count).fill(structure!.smiles).join(".")
                                    })
                                    .join(".")
                                await generateFromSmiles(smiles)
                            } catch (e) {
                                await message(String(e))
                            }
                        }}>自动配平电荷</Button>
                    </Box>
                </Grid2>
            </Box>
            <Box display={"flex"} flexDirection={"column"} gap={2}>
                <Typography variant="h6">相关结构</Typography>
                <Grid2 container spacing={2}>
                    <Box display={"flex"} justifyContent={"center"} alignItems={"stretch"} flexDirection={"row"} gap={2} flexWrap={"wrap"}>
                        {
                            state.relateds.map(([component, structure], index) => <ComponentItem key={index} component={component} structure={structure!} callback={reloadLinks} ro></ComponentItem>)
                        }
                    </Box>
                </Grid2>
            </Box>
        </Box>
    );
}

function ComponentItem(props: { component: Component, structure: Structure, callback: () => void, ro: boolean }) {
    const navigate = useNavigate();
    const { structure, component: stored } = props;
    const [count, setCount] = useState(stored.count)
    const [detail] = useFetch(() => getStructureDetail(structure.id), [structure, null, null, [], []], [structure.id])
    const image = detail[2];
    // 数目可能被外部改动（例如自动配平电荷），需要跟随最新的值
    useEffect(() => {
        setCount(stored.count)
    }, [stored.count])
    useEffect(() => {
        setComponent(stored.structure_id, stored.component_id, count).then(props.callback)
    }, [count])
    return <Box gap={1} width={256} display={"flex"} flexDirection={"column"} alignItems={"stretch"} justifyContent={"stretch"}>
        <Box height={256} display={"flex"} alignItems={"center"} justifyContent={"center"}>{
            image !== null ? <img style={{ maxWidth: "100%", maxHeight: "100%", objectFit: "contain" }} src={URL.createObjectURL(new Blob([Uint8Array.from(image.image)], { type: mime.getType(image.filename) ?? `image/png` }))}></img> : <Typography>图像未上传</Typography>
        }</Box>
        {structure.name !== null ? <Typography>名称：{structure.name}</Typography> : null}
        <Typography>分子式：{structure.formula}</Typography>
        {structure.smiles !== null ? <Typography>SMILES：{structure.smiles}</Typography> : null}
        {structure.charge !== null ? <Typography>电荷：{structure.charge}</Typography> : null}
        <TextField fullWidth label="数量" value={count} onChange={(e) => setCount(Number(e.target.value))}></TextField>
        {props.ro ? null : <Button variant="contained" color="error" onClick={() => deleteComponent(stored.structure_id, stored.component_id).then(props.callback)}>删除</Button>}
        <Button variant="contained" color="info" onClick={() => navigate(`/structure?id=${structure.id}`)}>查看</Button>
    </Box>
}
