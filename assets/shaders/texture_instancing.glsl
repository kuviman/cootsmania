varying vec2 v_vt;
varying vec4 v_color;

#ifdef VERTEX_SHADER
attribute vec2 a_pos;
attribute vec4 i_color;
attribute mat3 i_mat;
uniform mat3 u_projection_matrix;
uniform mat3 u_view_matrix;
void main() {
    v_vt = a_pos;
    v_color = i_color;
    vec3 pos = u_projection_matrix * u_view_matrix * i_mat * vec3(a_pos * 2.0 - 1.0, 1.0);
    gl_Position = vec4(pos.xy, 0.0, pos.z);
}
#endif

#ifdef FRAGMENT_SHADER
uniform sampler2D u_texture;
void main() {
    gl_FragColor = texture2D(u_texture, v_vt) * v_color;
}
#endif