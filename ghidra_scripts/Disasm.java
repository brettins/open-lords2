// Print disassembly. args: hexAddr count
//@category Lords2
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Instruction;

public class Disasm extends GhidraScript {
    @Override public void run() throws Exception {
        String[] a = getScriptArgs();
        Address addr = toAddr(a[0]);
        int n = Integer.parseInt(a[1]);
        Instruction ins = getInstructionAt(addr);
        for (int i = 0; i < n && ins != null; i++) {
            println(ins.getAddress() + "  " + ins.toString());
            ins = ins.getNext();
        }
    }
}
