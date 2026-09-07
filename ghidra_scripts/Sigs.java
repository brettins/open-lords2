// Print current signature of each function (hex addrs as args).
//@category Lords2
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class Sigs extends GhidraScript {
    @Override public void run() throws Exception {
        for (String a : getScriptArgs()) {
            Function f = getFunctionContaining(toAddr(a));
            println(a + "  " + (f == null ? "(no function)" :
                f.getEntryPoint() + " " + f.getName() + "  ::  " + f.getSignature().getPrototypeString()));
        }
    }
}
