import { DependencyList, useState, useEffect } from "react";

export default function useFetch<T, D extends DependencyList>(fetcher: () => Promise<T>, defaultValue: T, dependency: D) {
    const [value, setValue] = useState<T>(defaultValue);
    useEffect(() => {
        fetcher().then(value => {
            setValue(value)
        })
    }, dependency)
    const refresh = () => fetcher().then(value => {
        setValue(value)
    })
    return [value, refresh] as const
}