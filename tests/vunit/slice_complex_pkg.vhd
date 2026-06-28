library ieee;
use ieee.std_logic_1164.all;

package slice_complex_pkg is

component streamlet_com
  port(
    clk : in std_logic;
    rst : in std_logic;
    in_pass_valid : in std_logic;
    in_pass_ready : out std_logic;
    in_pass_data : in std_logic_vector(263 downto 0);
    in_pass_stai : in std_logic_vector(2 downto 0);
    in_pass_endi : in std_logic_vector(2 downto 0);
    in_pass_strb : in std_logic_vector(7 downto 0);
    in_pass2_valid : in std_logic;
    in_pass2_ready : out std_logic;
    in_pass2_data : in std_logic_vector(117 downto 0);
    in_pass2_strb : in std_logic_vector(0 downto 0);
    out_pass_valid : out std_logic;
    out_pass_ready : in std_logic;
    out_pass_data : out std_logic_vector(263 downto 0);
    out_pass_stai : out std_logic_vector(2 downto 0);
    out_pass_endi : out std_logic_vector(2 downto 0);
    out_pass_strb : out std_logic_vector(7 downto 0)
  );
end component;

component slice_complex_a
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
end component;

end package slice_complex_pkg;