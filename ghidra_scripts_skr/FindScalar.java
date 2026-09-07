// Find instructions whose operands contain a given scalar value. Args: hex values.
//@category Lords2
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
import ghidra.program.model.scalar.Scalar;

public class FindScalar extends GhidraScript {
    @Override public void run() throws Exception {
        for (String a : getScriptArgs()) {
            long want = Long.parseLong(a, 16);
            println("### scalar 0x" + Long.toHexString(want) + " (" + want + ")");
            InstructionIterator it = currentProgram.getListing().getInstructions(true);
            int n = 0;
            while (it.hasNext() && n < 60) {
                Instruction ins = it.next();
                for (int i = 0; i < ins.getNumOperands(); i++) {
                    Object[] o = ins.getOpObjects(i);
                    for (Object x : o) {
                        if (x instanceof Scalar && ((Scalar) x).getUnsignedValue() == want) {
                            Function f = getFunctionContaining(ins.getAddress());
                            println("  " + ins.getAddress() + "  " + ins
                                + "   in " + (f == null ? "(none)" : f.getEntryPoint() + " " + f.getName()));
                            n++;
                        }
                    }
                }
            }
        }
    }
}
