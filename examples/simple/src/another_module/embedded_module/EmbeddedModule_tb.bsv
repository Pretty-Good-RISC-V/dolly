//!topmodule mkEmbeddedModule_tb
import EmbeddedModule::*;

module mkEmbeddedModule_tb(Empty);
    let em <- mkEmbeddedModule();

    rule execute;
        $display(">>>PASS");
        $finish();
    endrule
endmodule
