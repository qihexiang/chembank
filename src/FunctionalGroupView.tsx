import { useEffect, useState } from "react";
import { Box, Button, Grid2, TextField, Typography } from "@mui/material";
import { message } from "@tauri-apps/api/dialog";
import { createFunctionalGroup, FunctionalGroup, listFunctionalGroups, rematchFunctionalGroups, removeFunctionalGroup } from "./bindings";

export default function FunctionalGroupView() {
    const [groups, setGroups] = useState<FunctionalGroup[]>([]);
    const [name, setName] = useState("");
    const [smarts, setSmarts] = useState("");

    const refresh = () => listFunctionalGroups().then(setGroups).catch(e => message(String(e)));

    useEffect(() => {
        refresh()
    }, [])

    return <Box display={"flex"} flexDirection={"column"} gap={2}>
        <Typography variant="h5">官能团</Typography>
        <Typography>新增的官能团会用 SMARTS 匹配库中全部结构，之后“按官能团检索”直接使用匹配结果。</Typography>
        <Box display={"flex"} flexDirection={"row"} gap={2} alignItems={"center"}>
            <TextField label="名称" placeholder="例如：硝酸酯" value={name} onChange={(e) => setName(e.target.value)}></TextField>
            <TextField sx={{ width: 384 }} label="SMARTS" placeholder="例如：[OX2][NX3+](=O)[O-]" value={smarts} onChange={(e) => setSmarts(e.target.value)}></TextField>
            <Button variant="contained" color="success" onClick={async () => {
                try {
                    await createFunctionalGroup(name, smarts)
                    setName("")
                    setSmarts("")
                    await refresh()
                } catch (e) {
                    await message(String(e))
                }
            }}>添加并匹配全部结构</Button>
            <Button variant="contained" color="warning" onClick={async () => {
                try {
                    await rematchFunctionalGroups()
                    await message("已按当前词表重算全部结构的官能团")
                } catch (e) {
                    await message(String(e))
                }
            }}>重新匹配全部结构</Button>
        </Box>
        <Grid2 container spacing={1}>
            <Grid2 size={3}><Typography variant="h6">名称</Typography></Grid2>
            <Grid2 size={7}><Typography variant="h6">SMARTS</Typography></Grid2>
            {
                groups.map(group => <Grid2 container size={12} spacing={1} alignItems={"center"} key={group.id}>
                    <Grid2 size={3}>{group.name}</Grid2>
                    <Grid2 size={7}><Typography sx={{ fontFamily: "monospace" }}>{group.smarts}</Typography></Grid2>
                    <Grid2 size={2}>
                        <Button variant="contained" color="error" onClick={async () => {
                            try {
                                await removeFunctionalGroup(group.id)
                                await refresh()
                            } catch (e) {
                                await message(String(e))
                            }
                        }}>删除</Button>
                    </Grid2>
                </Grid2>)
            }
        </Grid2>
    </Box>
}
