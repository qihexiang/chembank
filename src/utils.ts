const formulaRE = /([A-Z][a-z]?)([1-9][0-9]*)?/g;

export function analyseMoleculeFormula(formula: string) {
    const matches = formula.matchAll(formulaRE);
    const atoms: Record<string, number> = {};
    for (const matched of matches) {
        const sym = matched[1];
        const amount = matched[2] == "" ? 1 : parseInt(matched[2]);
        atoms[sym] = amount;
    }
    return atoms;
}

export function explosionSimulate(atoms: Record<string, number>) {
    let freeOxygens = atoms["O"] ?? 0;
    const carbons = atoms["C"];
    const oxygenEnoughForCarbons = carbons <= 2 * freeOxygens;
    const carbonDioxides = oxygenEnoughForCarbons ? carbons : freeOxygens / 2;
    freeOxygens -= carbonDioxides * 2;
    const hydrogens = atoms["H"];
    const oxygenEnoughForHydrogens = hydrogens <= freeOxygens / 2;
    const water = oxygenEnoughForHydrogens ? hydrogens / 2 : freeOxygens;
    freeOxygens -= water;
    const carbonRest = carbons - carbonDioxides;
    const carbonOxides = Math.min(carbonRest, freeOxygens);
    freeOxygens -= carbonOxides;
    const hydrogenGas = hydrogens / 2 - water;
    const nitrogenGas = atoms["N"] / 2;
    const oxygens = freeOxygens / 2;
    return {
        CO2: carbonDioxides,
        H2O: water,
        CO: carbonOxides,
        H2: hydrogenGas,
        N2: nitrogenGas,
        O2: oxygens,
    } as const;
}

const atomicMasses = {
    "H": 1.008,
    "He": 4.0026,
    "Li": 6.94,
    "Be": 9.012,
    "B": 10.81,
    "C": 12.01,
    "N": 14.01,
    "O": 16.00,
    "F": 19.00,
    "Ne": 20.18,
    "Na": 22.99,
    "Mg": 24.31,
    "Al": 26.98,
    "Si": 28.09,
    "P": 30.97,
    "S": 32.07,
    "Cl": 35.45,
    "Ar": 39.95,
    "K": 39.10,
    "Ca": 40.08,
    "Sc": 44.96,
    "Ti": 47.87,
    "V": 50.94,
    "Cr": 52.00,
    "Mn": 54.94,
    "Fe": 55.85,
    "Co": 58.93,
    "Ni": 58.69,
    "Cu": 63.55,
    "Zn": 65.38,
    "Ga": 69.72,
    "Ge": 72.64,
    "As": 74.92,
    "Se": 78.96,
    "Br": 79.90,
    "Kr": 83.80,
    "Rb": 85.47,
    "Sr": 87.62,
    "Y": 88.91,
    "Zr": 91.22,
    "Nb": 92.91,
    "Mo": 95.94,
};

function moleculeWeight(atoms: Record<string, number>) {
    let total = 0;
    for (let [atom, amount] of Object.entries(atoms)) {
        total += amount * atomicMasses[atom as keyof typeof atomicMasses];
    }
    return total;
}

export function calculateNMQ(
    atoms: Record<string, number>,
    gas: ReturnType<typeof explosionSimulate>,
    reactantH: number, // 单位为kJ/mol
) {
    const gasMols = Object.values(gas).reduce(
        (current, next) => current + next,
        0.0,
    );
    const molWeight = moleculeWeight(atoms);
    const gasWeight = calculateGasWeight(gas);
    const dH = - (reactantH / 4.184) + calculateGasH(gas);
    return [gasMols / molWeight, gasWeight / gasMols, -dH / molWeight * 1000];
}

function calculateGasWeight(gas: ReturnType<typeof explosionSimulate>) {
    return gas.CO2 * (atomicMasses["C"] + 2 * atomicMasses["O"]) +
        gas.CO * (atomicMasses["C"] + atomicMasses["O"]) +
        gas.H2 * 2 * atomicMasses["H"] +
        gas.H2O * (2 * atomicMasses["H"] + atomicMasses["O"]) +
        gas.N2 * 2 * atomicMasses["N"] +
        gas.O2 * 2 * atomicMasses["O"];
}

function calculateGasH(gas: ReturnType<typeof explosionSimulate>) {
    return gas.CO2 * (-94.05) +
        gas.H2O * (-57.80) +
        gas.CO  * (- 26.42);
}

export function calculateDP(N: number, M: number, Q: number, density: number) {
    return [
        1.01 * Math.sqrt(N * Math.sqrt(M * Q)) * (1 + 1.30 * density) * 1000,
        1.558 * Math.pow(density, 2) * N * Math.sqrt(M * Q),
    ];
}

const formula = "C3H9N9O7";

const atoms = analyseMoleculeFormula(formula);
const gas = explosionSimulate(atoms);
console.log(atoms)
console.log(calculateGasH(gas))
// console.log(- 748.5176 /4.184 + calculateGasH(gas))
// const[ N,M,Q] = calculateNMQ(atoms, gas, 748.5176)
// console.log(N,M,Q);
// console.log(calculateDP(N,M,Q,1.71))
