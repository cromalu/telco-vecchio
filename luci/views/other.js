'use strict';

'require view';
'require ui';
'require form';
'require rpc';
'require fs';

return view.extend({

    render: function() {
        var mapdata = { other: {}};
        var m = new form.JSONMap(mapdata, _('Other Actions'));
        var s = m.section(form.NamedSection, 'other', _('Other'));

	    //SSMTP config
        o = s.option(form.SectionValue, 'other', form.NamedSection, 'other', 'other', _('SSMTP Configuration'), _('Check and update SSMTP configuration file'));
        var ss = o.subsection;
       var o = ss.option(form.Button, 'download', _('Download current SSMTP configuration file'), _('In case you need to check the SSMTP configuration'));
        o.inputtitle = _('Download');
       o.onclick = L.bind(function(ev) {
           var form = E('form', {
               'method': 'post',
               'action': L.env.cgi_base + '/cgi-download',
               'enctype': 'application/x-www-form-urlencoded'
           }, [
               E('input', { 'type': 'hidden', 'name': 'sessionid', 'value': rpc.getSessionID() }),
               E('input', { 'type': 'hidden', 'name': 'path',      'value': '/etc/ssmtp/ssmtp.conf' }),
               E('input', { 'type': 'hidden', 'name': 'filename',  'value': 'ssmpt.conf'})
           ]);
          ev.currentTarget.parentNode.appendChild(form);
          form.submit();
          form.parentNode.removeChild(form);
       }, this);
        var o = ss.option(form.Button, 'upload', _('Upload a new SSMTP configuration file'), _('In case you need to change the SSMTP configuration'));
        o.inputtitle = _('Upload');
        o.onclick = L.bind(function(ev) {
            return ui.uploadFile('/etc/ssmtp/ssmpt.conf',ev).then(function() {
                   ui.showModal(_('Success'), [
                       E('p', [ _('SSMTP Configuration file uploaded successfuly') ]),
                       E('div', { 'class': 'right' }, [
                           E('button', { 'click': ui.hideModal }, [ _('OK') ])
                       ])]);
            }).catch(function(err) {
                ui.addNotification(null, E('p', [ _('SSMTP Configuration file upload failed: %s').format(err.message) ]));
            });
        }, this);

        //SSMTP reverse aliases
        o = s.option(form.SectionValue, 'other', form.NamedSection, 'other', 'other', _('SSMTP Reverse Aliases'), _('Check and update SSMTP reverse aliases'));
        var ss = o.subsection;
        var o = ss.option(form.Button, 'download', _('Download current SSMTP reverse aliases'), _('In case you need to check the SSMTP reverse aliases'));
        o.inputtitle = _('Download');
        o.onclick = L.bind(function(ev) {
            var form = E('form', {
                'method': 'post',
                'action': L.env.cgi_base + '/cgi-download',
                'enctype': 'application/x-www-form-urlencoded'
            }, [
                E('input', { 'type': 'hidden', 'name': 'sessionid', 'value': rpc.getSessionID() }),
                E('input', { 'type': 'hidden', 'name': 'path',      'value': '/etc/ssmtp/revaliases' }),
                E('input', { 'type': 'hidden', 'name': 'filename',  'value': 'revaliases'})
            ]);
            ev.currentTarget.parentNode.appendChild(form);
            form.submit();
            form.parentNode.removeChild(form);
        }, this);
        var o = ss.option(form.Button, 'upload', _('Upload new SSMTP Reverse Aliases'), _('In case you need to change SSMTP Reverse Aliases'));
        o.inputtitle = _('Upload');
        o.onclick = L.bind(function(ev) {
            return ui.uploadFile('/etc/ssmtp/revaliases',ev).then(function() {
                ui.showModal(_('Success'), [
                    E('p', [ _('SSMTP Reverse Aliases file uploaded successfuly') ]),
                    E('div', { 'class': 'right' }, [
                        E('button', { 'click': ui.hideModal }, [ _('OK') ])
                    ])]);
            }).catch(function(err) {
                ui.addNotification(null, E('p', [ _('SSMTP Reverse Aliases upload failed: %s').format(err.message) ]));
            });
        }, this);

        //SSH key config
        o = s.option(form.SectionValue, 'other', form.NamedSection, 'other', 'other', _('SSH Key Management'), _('Check and update SSH key'));
        var ss = o.subsection;
        var o = ss.option(form.Button, 'download', _('Download current SSH key file'), _('In case you need to check the SSH key'));
        o.inputtitle = _('Download');
        o.onclick = L.bind(function(ev) {
            var form = E('form', {
                'method': 'post',
                'action': L.env.cgi_base + '/cgi-download',
                'enctype': 'application/x-www-form-urlencoded'
            }, [
                E('input', { 'type': 'hidden', 'name': 'sessionid', 'value': rpc.getSessionID() }),
                E('input', { 'type': 'hidden', 'name': 'path',      'value': '/etc/dropbear/dropbear_rsa_host_key' }),
                E('input', { 'type': 'hidden', 'name': 'filename',  'value': 'key'})
            ]);
            ev.currentTarget.parentNode.appendChild(form);
            form.submit();
            form.parentNode.removeChild(form);
        }, this);
        var o = ss.option(form.Button, 'upload', _('Upload a new SSH key file'), _('In case you need to change the SSH key'));
        o.inputtitle = _('Upload');
        o.onclick = L.bind(function(ev) {
            return ui.uploadFile('/etc/dropbear/dropbear_rsa_host_key',ev).then(function() {
                ui.showModal(_('Success'), [
                    E('p', [ _('SSH key uploaded successfuly') ]),
                    E('div', { 'class': 'right' }, [
                        E('button', { 'click': ui.hideModal }, [ _('OK') ])
                    ])]);
            }).catch(function(err) {
                ui.addNotification(null, E('p', [ _('SSH key file upload failed: %s').format(err.message) ]));
            });
        }, this);


        return m.render();
    }
});
