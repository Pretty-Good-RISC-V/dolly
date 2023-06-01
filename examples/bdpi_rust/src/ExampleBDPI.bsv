//!extra_library ./target/debug/libexample_bdpi.a
import "BDPI" function Bit#(32) bdpi_function(Bit#(8) value);

interface ExampleBDPI_Ifc;
    method Bit#(32) call(Bit#(8) value);
endinterface

module mkExampleBDPI(ExampleBDPI_Ifc);
    method Bit#(32) call(Bit#(8) value);
        return bdpi_function(value);
    endmethod
endmodule
