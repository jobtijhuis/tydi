library ieee;
use ieee.std_logic_1164.all;

library work;
use work.slice_complex_pkg.all;
use work.Stream_pkg.all;

entity slice_complex_a is
  port(
    clk : in std_logic;
    rst : in std_logic;
    in_valid : in std_logic;
    in_ready : out std_logic;
    in_data : in std_logic_vector(263 downto 0);
    in_stai : in std_logic_vector(2 downto 0);
    in_endi : in std_logic_vector(2 downto 0);
    in_strb : in std_logic_vector(7 downto 0);
    out_valid : out std_logic;
    out_ready : in std_logic;
    out_data : out std_logic_vector(263 downto 0);
    out_stai : out std_logic_vector(2 downto 0);
    out_endi : out std_logic_vector(2 downto 0);
    out_strb : out std_logic_vector(7 downto 0)
  );
end entity slice_complex_a;

architecture behavioral of slice_complex_a is
  signal clk_wire : std_logic;
  signal reset_wire : std_logic;
  signal in_valid_wire : std_logic;
  signal in_ready_wire : std_logic;
  signal in_data_wire : std_logic_vector(277 downto 0);
  signal out_valid_wire : std_logic;
  signal out_ready_wire : std_logic;
  signal out_data_wire : std_logic_vector(277 downto 0);
begin
  clk_wire <= clk;
  reset_wire <= rst;
  in_valid_wire <= in_valid;
  in_ready <= in_ready_wire;
  in_data_wire <= in_data & in_stai & in_endi & in_strb;
  out_valid <= out_valid_wire;
  out_ready_wire <= out_ready;
  out_data <= out_data_wire(277 downto 14);
  out_stai <= out_data_wire(13 downto 11);
  out_endi <= out_data_wire(10 downto 8);
  out_strb <= out_data_wire(7 downto 0);
  canonical: StreamSlice
    generic map(
      DATA_WIDTH => 278
    )
    port map(
      clk => clk_wire,
      reset => reset_wire,
      in_valid => in_valid_wire,
      in_ready => in_ready_wire,
      in_data => in_data_wire,
      out_valid => out_valid_wire,
      out_ready => out_ready_wire,
      out_data => out_data_wire
    );
end architecture behavioral;
